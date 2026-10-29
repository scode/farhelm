//! Show host-owned archives without treating a stale listing as deletion authority.
//!
//! Counts are event-driven. Opening the dialog asks for bounded disk usage;
//! confirmations and immediate single deletes freeze IDs and connection incarnations, while the supervisor
//! rechecks ownership before any mutation. Read generations prevent an earlier
//! count/measurement from restoring entries removed by this client's cleanup.
//! The list gains a named tab stop when none of its actions can take focus,
//! so keyboard users can still scroll retained host information in WebKit
//! while a read or deletion is pending.

use crate::anchored_popovers::{AnchorRegion, MovementOwner, use_rendered_movement};
use crate::hosts::HostsRead;
use crate::icons::{BranchIcon, TrashIcon};
use crate::modal_dialog::{
    DialogFocus, DialogHandle, ModalDialog, RecoveryDelay, RecoveryFocus, ScrollRegion,
};
use crate::ops::{ConfirmSlot, ConfirmToken, OpGuard, PaneGate, use_confirm_slot};
use crate::peer::display_peer;
use crate::{ApiBase, Host, HostId, HostPhase};
use dioxus::prelude::*;
use farhelm_proto::{CheckoutTrashIssue, CheckoutTrashListing};
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use web_time::Instant;

const DIALOG: &str = ".checkout-trash-dialog";
/// Only whole-dialog dismissal returns here; cancelling a confirmation stays inside.
const RETURN_FOCUS: &str =
    "document.querySelector('.checkout-trash-button')?.focus({preventScroll:true});";
// The host rejects larger batches; disclose this rather than silently omitting
// a host from a global destructive confirmation.
const MAX_SELECTION: usize = 10_000;
/// How long per-entry deletes stay disabled after the dialog's rows move.
///
/// An entry delete has no confirmation, and a re-read, a host change or the
/// dialog's own delete can move rows under a resting pointer; a click already
/// on its way, or a double-click's second click, would then permanently delete
/// the checkout that moved there. 700 ms matches the approval cards' answer
/// pause for the same hazard (SPEC.md, "Agent-spawned sessions").
const SETTLE_MS: u64 = 700;

/// Count wording stays grammatical at the destructive confirmation boundary.
fn checkouts(count: usize) -> &'static str {
    if count == 1 { "checkout" } else { "checkouts" }
}

/// Use the same optional decimal text for the header span and its width key.
/// Zero has no span or gap; 1→2 keeps its monospace width, while 0→1 and 9→10
/// can wrap the heading and move every session anchor below it.
fn sidebar_count_text(count: usize) -> Option<String> {
    (count > 0).then(|| count.to_string())
}

/// Keep the can and its descriptions honest about archives not yet inspected.
/// Checked rows remain an exact count; diagnostics never become checkout counts.
fn sidebar_summary(count: usize, has_unchecked: bool, all_ready: bool) -> (bool, String) {
    let full = count > 0 || has_unchecked;
    let label = if !all_ready || has_unchecked {
        format!(
            "{count} known archived {}, some entries or hosts unchecked",
            checkouts(count)
        )
    } else if count == 0 {
        "empty".into()
    } else {
        format!("{count} archived {}", checkouts(count))
    };
    (full, label)
}

/// A reply is meaningful only on the connection that supplied it. The retained
/// same-connection reply keeps the count steady during a fresh measurement.
#[derive(Clone, Default)]
struct HostTrash {
    incarnation: u64,
    reply: Option<Result<CheckoutTrashListing, String>>,
    loading: bool,
}

/// A deletion set never grows with a subsequent listing or a new archive.
/// Bulk selections include diagnostic IDs: cleanup rechecks them and may delete
/// a now-verifiable archive or forget an unusable record without touching its
/// folder. Immediate entry deletion selects only a displayed checkout.
#[derive(Clone, PartialEq)]
struct Target {
    host: HostId,
    incarnation: u64,
    name: String,
    ids: Vec<String>,
    count: usize,
    unchecked: usize,
    bytes: u64,
    partial: bool,
}

/// What one in-place confirmation band shows and what confirming it deletes:
/// the band it is drawn in (a host's, or `None` for "Delete all" at the top
/// of the host list) and the targets frozen when it opened.
///
/// It is the key of the dialog's [`ConfirmSlot`], so every opening's
/// [`ConfirmToken`] carries exactly the targets its band displayed. The band
/// draws its counts, sizes and host names from the token, and a confirmed
/// answer deletes the token's targets, never a selection recomputed from the
/// listing when the click runs.
#[derive(Clone, PartialEq)]
struct Confirmation {
    host: Option<HostId>,
    targets: Vec<Target>,
}

/// Where a deletion run's targets come from, which decides how starting the
/// run treats the confirmation band.
enum Selection {
    /// One displayed checkout, deleted without a confirmation (an entry's
    /// own Delete). Its targets come from the click-time host by design, and
    /// starting it closes whatever band is open.
    Entry(Target),
    /// The band opening the user confirmed, as the render that drew its
    /// "Delete permanently" button saw it. The run deletes exactly the
    /// targets that opening froze, and starting it consumes the opening.
    Band(ConfirmToken<Confirmation>),
}

impl Selection {
    /// The targets this run would delete, or `None` for an answer to a band
    /// opening that is no longer on screen.
    ///
    /// A band opening is gone once it was cancelled, confirmed, or replaced,
    /// including by a band with identical content opened after a cancel: an
    /// answer queued in the same event burst was drawn for the earlier
    /// showing, which the user is no longer looking at. Such an answer must
    /// change nothing, the band now on screen included, so callers check this
    /// before any step that could close the band or refresh the listing.
    fn targets(&self, slot: &ConfirmSlot<Confirmation>) -> Option<Vec<Target>> {
        match self {
            Self::Entry(target) => Some(vec![target.clone()]),
            Self::Band(answered) => slot
                .is_current(answered)
                .then(|| answered.key().targets.clone()),
        }
    }

    /// Close the band as the run starts: consume the confirmed opening, or,
    /// for an entry delete, clear whatever band is open. Returns `false` only
    /// when a confirmed opening is no longer the open one, in which case the
    /// run must not start.
    ///
    /// Called after the run has claimed the page, so a confirmation refused
    /// for a busy page leaves its band up to be answered again.
    fn commit(&self, slot: &mut ConfirmSlot<Confirmation>) -> bool {
        match self {
            Self::Entry(_) => {
                slot.clear();
                true
            }
            Self::Band(answered) => slot.take(answered).is_some(),
        }
    }
}

/// Frozen per-host progress survives until the last host finishes, so a fast
/// host does not lose its denominator while another host is still deleting.
/// The cursor advances on every confirmed reply, including a per-record issue;
/// only confirmed record removals increment `deleted`.
#[derive(Clone)]
struct HostDeletion {
    /// Constant-time row lookup avoids scanning the frozen selection for every
    /// displayed checkout. The task retains the ordered IDs for dispatch.
    positions: HashMap<String, usize>,
    /// Targets put displayed checkouts before diagnostic-only IDs. Keep their
    /// count separate so unchecked record cleanup never inflates checkout totals.
    checkouts: usize,
    next: usize,
    started_at: Option<Instant>,
    deleted: usize,
    finished: bool,
}

/// Presence holds the page claim across all per-host queues. Only the final
/// host completion clears the run and triggers a single listing refresh.
#[derive(Clone)]
struct DeletionRun {
    hosts: HashMap<HostId, HostDeletion>,
}

impl DeletionRun {
    /// Describe outstanding requests, excluding queues stopped by a lost reply.
    /// Diagnostic-only records remain visible as unchecked entries, not checkouts.
    fn status(&self) -> String {
        let (checkouts_left, unchecked_left) = self
            .hosts
            .values()
            .filter(|host| !host.finished)
            .fold((0, 0), |(checkouts_left, unchecked_left), host| {
                (
                    checkouts_left + host.checkouts.saturating_sub(host.next),
                    unchecked_left
                        + host
                            .positions
                            .len()
                            .saturating_sub(host.next.max(host.checkouts)),
                )
            });
        let status = format!(
            "Deleting… {checkouts_left} {} left",
            checkouts(checkouts_left)
        );
        if unchecked_left == 0 {
            status
        } else {
            format!("{status} · {unchecked_left} unchecked entries left")
        }
    }
}

/// Mount only for the current request. Its monotonic origin belongs to dispatch,
/// not render; the component's timer is cancelled when that request leaves the row.
#[component]
fn EntryProgress(started_at: Instant) -> Element {
    let mut elapsed = use_signal(|| started_at.elapsed().as_secs());
    use_future(move || async move {
        loop {
            crate::reader::sleep_ms(1_000).await;
            elapsed.set(started_at.elapsed().as_secs());
        }
    });
    rsx! { span { class: "trash-entry-progress",
        span { class: "delete-spinner", aria_hidden: "true" }
        span { class: "trash-entry-elapsed", "{elapsed()}s" }
    } }
}

/// Only connected rows can contribute current archives or accept cleanup.
fn connected(host: &Host) -> bool {
    matches!(host.state, HostPhase::Connected { .. }) && host.incarnation != 0
}

/// Count unique deletion IDs, preserving repeated diagnostics only for display.
fn selection_size(listing: &CheckoutTrashListing) -> usize {
    listing
        .checkouts
        .iter()
        .map(|entry| &entry.id)
        .chain(listing.issues.iter().map(|issue| &issue.id))
        .collect::<HashSet<_>>()
        .len()
}

/// Keep the host header and frozen confirmation honest about unchecked archives.
/// Repeated diagnostics count once per ID, and their unmeasured bytes make the
/// known byte sum partial even when every displayed checkout has a size.
struct ListingTotals {
    count: usize,
    unchecked: usize,
    bytes: u64,
    partial: bool,
}

impl ListingTotals {
    /// Distinguish checked checkouts from entries the host could not inspect.
    fn count_label(&self) -> String {
        let checked = format!("{} {}", self.count, checkouts(self.count));
        if self.unchecked == 0 {
            checked
        } else {
            let noun = if self.unchecked == 1 {
                "entry"
            } else {
                "entries"
            };
            format!("{checked} · {} unchecked {noun}", self.unchecked)
        }
    }
}

/// Summarize listing evidence without granting any deletion authority.
/// Eligibility still belongs to `target`, including the connection and batch limit.
fn listing_totals(listing: &CheckoutTrashListing) -> ListingTotals {
    ListingTotals {
        count: listing.checkouts.len(),
        unchecked: listing
            .issues
            .iter()
            .map(|issue| &issue.id)
            .collect::<HashSet<_>>()
            .len(),
        bytes: listing
            .checkouts
            .iter()
            .filter_map(|entry| entry.bytes)
            .fold(0, u64::saturating_add),
        partial: !listing.issues.is_empty()
            || listing.checkouts.iter().any(|entry| entry.bytes.is_none()),
    }
}

/// Missing replies, read failures and unknown sizes stay partial regardless of cleanup eligibility.
fn reply_partial(reply: Option<&Result<CheckoutTrashListing, String>>) -> bool {
    reply
        .and_then(|reply| reply.as_ref().ok())
        .is_none_or(|listing| listing_totals(listing).partial)
}

/// Collapse repeated diagnostic IDs into one selection, keeping every message
/// visible separately. Displayed checkouts precede diagnostic-only IDs, which
/// lets progress distinguish known checkout work from unchecked record cleanup.
/// Sizes with any unknown checkout remain explicitly partial.
fn target(host: &Host, state: Option<&HostTrash>) -> Option<Target> {
    let state = state.filter(|state| {
        connected(host) && state.incarnation == host.incarnation && !state.loading
    })?;
    let listing = state.reply.as_ref()?.as_ref().ok()?;
    let mut seen = HashSet::new();
    let ids: Vec<_> = listing
        .checkouts
        .iter()
        .map(|entry| &entry.id)
        .chain(listing.issues.iter().map(|issue| &issue.id))
        .filter(|id| seen.insert((*id).clone()))
        .cloned()
        .collect();
    if ids.is_empty() || ids.len() > MAX_SELECTION {
        return None;
    }
    let totals = listing_totals(listing);
    Some(Target {
        host: host.id,
        incarnation: host.incarnation,
        // Confirmation helpers escape this frozen name when they build their labels.
        name: host.name.raw().to_owned(),
        ids,
        count: totals.count,
        unchecked: totals.unchecked,
        bytes: totals.bytes,
        partial: totals.partial,
    })
}

/// Narrow a settled host selection to a displayed checkout without bypassing
/// the host's cleanup limit or connection fence. Issue-only records deliberately
/// remain available only through the bulk actions, which explain their cleanup.
fn entry_target(host: &Host, state: Option<&HostTrash>, id: &str) -> Option<Target> {
    let mut selected = target(host, state)?;
    let listing = state?.reply.as_ref()?.as_ref().ok()?;
    let checkout = listing.checkouts.iter().find(|entry| entry.id == id)?;
    selected.ids = vec![checkout.id.clone()];
    selected.count = 1;
    selected.unchecked = 0;
    selected.bytes = checkout.bytes.unwrap_or_default();
    selected.partial = checkout.bytes.is_none();
    Some(selected)
}

/// Human-readable allocated space, with units that do not imply byte precision
/// when a host could measure only part of its trash.
fn space(bytes: u64, partial: bool) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < units.len() {
        value /= 1024.0;
        unit += 1;
    }
    let size = if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", units[unit])
    };
    if partial {
        format!("{size} measured · partial")
    } else {
        size
    }
}

/// The archive rename's known UTC date; missing historical evidence stays
/// unknown instead of being replaced by the directory's modification time.
/// Only the timestamp is technical text: the prefix and unknown-time message
/// remain interface prose without changing their combined accessible wording.
fn archived_when(seconds: Option<i64>) -> Element {
    let Some(seconds) = seconds else {
        return rsx! { "Archive time unknown" };
    };
    let (year, month, day) = farhelm_proto::time::civil_from_days(seconds.div_euclid(86400));
    let time = seconds.rem_euclid(86400);
    let timestamp = format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        time / 3600,
        time % 3600 / 60
    );
    rsx! { "Archived " span { class: "font-mono", "{timestamp}" } }
}

/// Dismiss the band opening `shown`, the one the cancelling render drew
/// (its Cancel button, or the dialog's Close and Escape while it was shown).
/// A stale cancel, drawn for a band since answered or replaced, does nothing,
/// focus included.
///
/// Cancelling a global choice returns to its pinned trigger, including Escape.
/// The band lives in the scrolling body, so clearing it alone would discard the
/// focused Cancel button and leave keyboard users without a useful next action.
fn cancel_confirmation(
    mut confirmation: ConfirmSlot<Confirmation>,
    shown: &ConfirmToken<Confirmation>,
) {
    // One test-and-clear decides both whether the band closes and whether
    // focus moves, so a refused cancel cannot move focus either.
    if confirmation.take(shown).is_none() {
        return;
    }
    if shown.key().host.is_none() {
        document::eval(
            // A re-list can disable "Delete all" while the confirmation is
            // open; focusing a disabled button does nothing and would drop
            // focus out of the dialog, so fall back to Close.
            "requestAnimationFrame(() => (document.querySelector('.trash-delete-all:enabled') ?? document.querySelector('.trash-close'))?.focus({preventScroll:true}))",
        );
    }
}

/// What the dialog's Close button or Escape does, decided from what the
/// render that drew it showed.
enum CloseAction<'a> {
    /// Dismiss the band opening that render showed. Goes through
    /// [`cancel_confirmation`], so it does nothing if that opening has since
    /// been answered or replaced.
    Dismiss(&'a ConfirmToken<Confirmation>),
    /// Close the whole dialog: no band was shown, and none is open now.
    CloseDialog,
    /// Do nothing: no band was shown, but one has opened since. The Close or
    /// Escape was not drawn for that band, and closing the dialog would hide
    /// a question the user has not read yet.
    Nothing,
}

/// Decide what a Close or Escape does. `shown` is the band opening its
/// render read from the slot (shown even if its host's section was not
/// drawn), and `band_open_now` is whether any band is open when the event
/// runs.
///
/// A Close or Escape never acts on a band opened after its render: with a
/// band shown it can only dismiss that one, and with none shown it closes
/// the dialog only if the slot is still empty.
fn close_action(
    shown: Option<&ConfirmToken<Confirmation>>,
    band_open_now: bool,
) -> CloseAction<'_> {
    match shown {
        Some(shown) => CloseAction::Dismiss(shown),
        None if band_open_now => CloseAction::Nothing,
        None => CloseAction::CloseDialog,
    }
}

/// A host message stays escaped and direction-isolated, including a path that
/// was preserved or partially cleaned. Removed IDs do not suppress diagnostics.
#[component]
fn Issue(issue: CheckoutTrashIssue) -> Element {
    rsx! { div { class: "checkout-trash-issue", role: "status",
        if !issue.path.is_empty() { div { class: "peer-value font-mono", dir: "ltr", "{display_peer(&issue.path)}" } }
        div { class: "peer-value", dir: "ltr", "{display_peer(&issue.message)}" }
    } }
}

/// Confirm the frozen visible set in place, including unverified entries.
/// An unchecked entry can become verifiable after listing, so the same warning
/// must cover permanent deletion as well as record-only cleanup.
///
/// `shown` is the opening the parent's render read from the slot. The text
/// comes from its frozen targets, and both buttons hand that same token back,
/// so the parent can refuse an answer for an opening no longer on screen and
/// otherwise delete exactly what this band said.
#[component]
fn ConfirmBand(
    shown: ConfirmToken<Confirmation>,
    busy: bool,
    submit_disabled: bool,
    oncancel: EventHandler<ConfirmToken<Confirmation>>,
    onsubmit: EventHandler<ConfirmToken<Confirmation>>,
) -> Element {
    let confirm = shown.key();
    let count: usize = confirm.targets.iter().map(|target| target.count).sum();
    let unchecked: usize = confirm.targets.iter().map(|target| target.unchecked).sum();
    let names = confirm
        .targets
        .iter()
        .filter(|target| target.count > 0)
        .map(|target| display_peer(&target.name))
        .collect::<Vec<_>>()
        .join(", ");
    let unchecked_names = confirm
        .targets
        .iter()
        .filter(|target| target.unchecked > 0)
        .map(|target| display_peer(&target.name))
        .collect::<Vec<_>>()
        .join(", ");
    let unchecked_noun = if unchecked == 1 { "entry" } else { "entries" };
    let bytes = confirm
        .targets
        .iter()
        .map(|target| target.bytes)
        .fold(0, u64::saturating_add);
    let partial = confirm.targets.iter().any(|target| target.partial) || unchecked > 0;
    rsx! { div { class: "checkout-trash-confirm", role: "group", aria_label: "Confirm permanent checkout deletion",
        // A confirmation can name more hosts than fit in the body. Reveal the
        // confirming answer and its Cancel fallback without moving the
        // pinned header; the warning remains available by scrolling upwards.
        onmounted: move |_| { document::eval("requestAnimationFrame(() => { const target = document.querySelector('.checkout-trash-confirm .trash-confirm-submit:enabled') ?? document.querySelector('.checkout-trash-confirm .trash-confirm-cancel:enabled'); target?.focus({preventScroll:true}); target?.closest('.checkout-trash-actions')?.scrollIntoView({block:'nearest'}); })"); },
        if count > 0 { p { "Delete {count} archived {checkouts(count)} ({space(bytes, partial)}) on " span { class: "peer-value", dir: "ltr", "{names}" } "? Work in them that was never committed or pushed is lost permanently." } }
        if unchecked > 0 { p { "Attempt cleanup of {unchecked} unchecked {unchecked_noun} on {unchecked_names}. Farhelm deletes any of these it can verify when cleanup runs; work in them that was never committed or pushed is lost permanently. The rest stay in place and may be forgotten from the trash." } }
        div { class: "checkout-trash-actions",
            button { class: "btn btn-neutral trash-confirm-cancel", disabled: busy, onclick: { let shown = shown.clone(); move |_| oncancel.call(shown.clone()) }, "Cancel" }
            button { class: "btn btn-danger trash-confirm-submit", disabled: submit_disabled, onkeydown: crate::guard_confirmation_enter, onclick: move |_| onsubmit.call(shown.clone()), "Delete permanently" }
        }
    } }
}

/// Keep the button and its dialog mounted under the list owner, so a session
/// deletion or selection change cannot drop an in-flight cleanup task.
#[component]
pub(super) fn TrashControl(
    hosts: Signal<HostsRead>,
    revision: Signal<u64>,
    gate: PaneGate,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let popovers = use_context::<MovementOwner>();
    let mut open = use_signal(|| false);
    let mut state = use_signal(HashMap::<HostId, HostTrash>::new);
    let mut generation = use_signal(|| 0_u64);
    let mut refresh = use_signal(|| 0_u64);
    // The one confirmation band, keyed by what it shows. Answers name the
    // opening their render drew; see `Selection` and `cancel_confirmation`.
    let mut confirmation = use_confirm_slot::<Confirmation, ()>();
    let mut notices = use_signal(HashMap::<HostId, Vec<CheckoutTrashIssue>>::new);
    let mut deletion = use_signal(|| None::<DeletionRun>);
    // The claim outlives all host tasks and drops with this component on cancel.
    let mut claim = use_signal(|| None::<OpGuard>);
    // The layout the dialog last drew and whether entry deletes have settled
    // since it changed (see `SETTLE_MS`). The generation keeps an older timer
    // from settling a newer layout.
    let mut drawn = use_signal(|| None::<u64>);
    let mut settled = use_signal(|| true);
    let mut settle_generation = use_signal(|| 0_u64);
    let signature = use_memo(move || {
        let read = hosts.read();
        (
            read.hosts()
                .unwrap_or_default()
                .iter()
                .map(|host| (host.id, host.incarnation, connected(host)))
                .collect::<Vec<_>>(),
            read.hosts().is_some() && read.refresh_error().is_none(),
        )
    });
    let read_base = base.clone();
    use_effect(move || {
        let (signature, hosts_known) = signature();
        let _ = revision();
        let _ = refresh();
        let measure = open();
        if deletion().is_some() {
            return;
        }
        let current = generation.peek().wrapping_add(1);
        generation.set(current);
        if !hosts_known {
            return;
        }
        state
            .write()
            .retain(|id, _| signature.iter().any(|(host, _, _)| host == id));
        for (host, incarnation, reachable) in signature {
            if !reachable {
                state.write().remove(&host);
                continue;
            }
            {
                let mut states = state.write();
                let entry = states.entry(host).or_default();
                if entry.incarnation != incarnation {
                    entry.reply = None;
                }
                entry.incarnation = incarnation;
                entry.loading = true;
            }
            let base = read_base.clone();
            spawn(async move {
                let reply =
                    crate::api::fetch_checkout_trash(&base, host, incarnation, measure).await;
                if *generation.peek() != current {
                    return;
                }
                if let Some(entry) = state
                    .write()
                    .get_mut(&host)
                    .filter(|entry| entry.incarnation == incarnation)
                {
                    entry.reply = Some(reply);
                    entry.loading = false;
                }
            });
        }
    });
    let host_error = hosts.read().refresh_error().map(str::to_owned);
    let hosts_known = hosts.read().hosts().is_some() && host_error.is_none();
    let current_hosts = hosts.read().hosts().unwrap_or_default().to_vec();
    let (count, has_unchecked) = current_hosts
        .iter()
        .filter(|host| hosts_known && connected(host))
        .filter_map(|host| {
            state
                .read()
                .get(&host.id)
                .filter(|entry| entry.incarnation == host.incarnation)
                .and_then(|entry| entry.reply.as_ref())
                .and_then(|reply| reply.as_ref().ok())
                .map(|reply| (reply.checkouts.len(), !reply.issues.is_empty()))
        })
        .fold((0, false), |(count, unchecked), (known, issues)| {
            (count + known, unchecked || issues)
        });
    // Use the final displayed span after all accepted host/count gates. Its
    // width can wrap the heading above the session list, independent of totals.
    let count_text = sidebar_count_text(count);
    let count_columns = count_text.as_ref().map_or(0, String::len);
    use_rendered_movement(&count_columns, || {
        popovers.capture_region(AnchorRegion::Sessions)
    });
    let targets: Vec<_> = current_hosts
        .iter()
        .filter(|_| hosts_known)
        .filter_map(|host| target(host, state.read().get(&host.id)))
        .collect();
    // A global choice must cover every reachable host. Failed, pending, and
    // over-limit listings stay visible and cannot quietly disappear from it.
    let all_ready = hosts_known
        && current_hosts
            .iter()
            .filter(|host| connected(host))
            .all(|host| {
                state.read().get(&host.id).is_some_and(|entry| {
                    entry.incarnation == host.incarnation
                        && !entry.loading
                        && matches!(&entry.reply, Some(Ok(listing)) if selection_size(listing) <= MAX_SELECTION)
                })
            });
    let progress = deletion();
    let busy = progress.is_some();
    let mut gate = gate;
    let delete_base = base;
    // Both entry clicks and confirmed bulk choices cross the same authority,
    // operation-guard and completion boundary. Only selection/confirmation differ.
    let dispatch = use_callback(move |selection: Selection| {
        if deletion.peek().is_some() {
            return;
        }
        // A stale band answer stops here, before the host check below can
        // close the band now on screen or refresh the listing under it.
        let Some(targets) = selection.targets(&confirmation) else {
            return;
        };
        // An empty selection has no reply that could retire a run or its gate.
        if targets.is_empty() {
            return;
        }
        // Host changes invalidate a frozen selection before dispatch;
        // the helm repeats the incarnation fence for changes it sees first.
        let valid = {
            let hosts = hosts.peek();
            hosts.refresh_error().is_none()
                && targets.iter().all(|target| {
                    hosts.hosts().unwrap_or_default().iter().any(|host| {
                        host.id == target.host
                            && connected(host)
                            && host.incarnation == target.incarnation
                    })
                })
        };
        if !valid {
            confirmation.clear();
            refresh += 1;
            return;
        }
        let Some(guard) = gate.claim_guard() else {
            return;
        };
        // Nothing between the check above and here can reopen the band (this
        // handler is synchronous), so a band answer's take succeeds; the
        // guard is dropped, releasing the page, if it somehow did not.
        if !selection.commit(&mut confirmation) {
            return;
        }
        claim.set(Some(guard));
        deletion.set(Some(DeletionRun {
            hosts: targets
                .iter()
                .map(|target| {
                    (
                        target.host,
                        HostDeletion {
                            positions: target
                                .ids
                                .iter()
                                .enumerate()
                                .map(|(index, id)| (id.clone(), index))
                                .collect(),
                            checkouts: target.count,
                            next: 0,
                            started_at: None,
                            deleted: 0,
                            finished: false,
                        },
                    )
                })
                .collect(),
        }));
        generation += 1; // supersede every pre-delete count and size reply
        notices.write().clear();
        for target in targets {
            let base = delete_base.clone();
            spawn(async move {
                // A host owns one sequential queue; independent host tasks can
                // progress together. Never rediscover targets between replies.
                for (index, id) in target.ids.iter().enumerate() {
                    let valid = {
                        let hosts = hosts.peek();
                        hosts.refresh_error().is_none()
                            && hosts.hosts().unwrap_or_default().iter().any(|host| {
                                host.id == target.host
                                    && connected(host)
                                    && host.incarnation == target.incarnation
                            })
                    };
                    if !valid {
                        notices.write().entry(target.host).or_default().push(CheckoutTrashIssue {
                            id: String::new(), path: String::new(),
                            message: "The host connection changed; remaining cleanup was not sent. Rechecking the trash.".into(),
                        });
                        break;
                    }
                    if let Some(host) = deletion
                        .write()
                        .as_mut()
                        .and_then(|run| run.hosts.get_mut(&target.host))
                    {
                        host.next = index;
                        host.started_at = Some(Instant::now());
                    }
                    let result = crate::api::empty_checkout_trash(
                        &base,
                        target.host,
                        target.incarnation,
                        std::slice::from_ref(id),
                    )
                    .await;
                    match result {
                        Ok(reply) => {
                            // These removals are confirmed records, not guessed
                            // successful files. Always keep overlapping issues.
                            if let Some(entry) = state
                                .write()
                                .get_mut(&target.host)
                                .filter(|entry| entry.incarnation == target.incarnation)
                                && let Some(Ok(listing)) = entry.reply.as_mut()
                            {
                                listing
                                    .checkouts
                                    .retain(|entry| !reply.removed.contains(&entry.id));
                                listing
                                    .issues
                                    .retain(|entry| !reply.removed.contains(&entry.id));
                            }
                            if let Some(host) = deletion
                                .write()
                                .as_mut()
                                .and_then(|run| run.hosts.get_mut(&target.host))
                            {
                                host.deleted += usize::from(reply.removed.contains(id));
                                host.next = index + 1;
                                host.started_at = None;
                            }
                            notices
                                .write()
                                .entry(target.host)
                                .or_default()
                                .extend(reply.issues);
                        }
                        Err(error) => {
                            notices.write().entry(target.host).or_default().push(CheckoutTrashIssue {
                            id: String::new(), path: String::new(),
                            message: format!("The cleanup reply could not be confirmed; some folders may already be deleted. Rechecking the trash: {error}"),
                        });
                            // The server owns accepted work after a lost reply.
                            // Sending more on this host could hide that uncertainty.
                            break;
                        }
                    }
                }
                // Retain this host's outcome while peers finish; only the last
                // queue releases admission and permits listing reads again.
                let finished = {
                    let mut run = deletion.write();
                    if let Some(active) = run.as_mut() {
                        if let Some(host) = active.hosts.get_mut(&target.host) {
                            host.finished = true;
                            host.started_at = None;
                        }
                        if active.hosts.values().all(|host| host.finished) {
                            *run = None;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };
                if finished {
                    claim.set(None);
                    refresh += 1;
                    document::eval(
                        "requestAnimationFrame(() => document.querySelector('.trash-close')?.focus({preventScroll:true}))",
                    );
                }
            });
        }
    });
    // The band opening this render reads from the slot. Its buttons, and
    // the dialog's Close and Escape, answer only this opening. A host band
    // stays the shown opening even when its host's section is not drawn
    // (the host left the list, or a re-list left its section nothing to
    // show), so Close and Escape then
    // cancels that unseen band rather than closing the dialog.
    let shown = confirmation.current();
    // Fingerprint everything that decides where entry deletes are drawn (the
    // open band, the reachability error, deletion progress, and per host its
    // state, entries, issues and notices), and re-arm the pause in the same
    // render that moves rows, so no render shows a moved row with its delete
    // enabled.
    let mut layout = DefaultHasher::new();
    let band = shown.as_ref().map(|shown| shown.key().host);
    (band, host_error.is_some(), busy).hash(&mut layout);
    for host in &current_hosts {
        let states = state.read();
        let entry = states
            .get(&host.id)
            .filter(|entry| entry.incarnation == host.incarnation);
        let rows = match entry.and_then(|entry| entry.reply.as_ref()) {
            Some(Ok(listing)) => Some((
                listing.checkouts.iter().map(|c| &c.id).collect::<Vec<_>>(),
                listing.issues.len(),
            )),
            _ => None,
        };
        let pending = entry.is_none_or(|entry| entry.loading);
        let notices = notices.read().get(&host.id).map(Vec::len);
        let deleting = progress
            .as_ref()
            .is_some_and(|run| run.hosts.contains_key(&host.id));
        (host.id, connected(host), pending, rows, notices, deleting).hash(&mut layout);
    }
    let layout = layout.finish();
    if *drawn.peek() != Some(layout) {
        drawn.set(Some(layout));
        let generation = settle_generation.peek().wrapping_add(1);
        settle_generation.set(generation);
        settled.set(false);
        spawn(async move {
            crate::reader::sleep_ms(SETTLE_MS).await;
            if *settle_generation.peek() == generation {
                settled.set(true);
            }
        });
    }
    // Close and Escape act on what their render read; see `close_action`.
    let close = {
        let shown = shown.clone();
        move || {
            if deletion.peek().is_some() {
                return;
            }
            match close_action(shown.as_ref(), confirmation.peek_key().is_some()) {
                CloseAction::Dismiss(shown) => cancel_confirmation(confirmation, shown),
                CloseAction::Nothing => {}
                CloseAction::CloseDialog => {
                    DialogHandle::new(DIALOG).close(RETURN_FOCUS);
                    open.set(false);
                }
            }
        }
    };
    // Both bands (global and per-host) answer through these, each handing
    // back the token its render drew.
    let band_cancel =
        move |shown: ConfirmToken<Confirmation>| cancel_confirmation(confirmation, &shown);
    let band_submit =
        move |answered: ConfirmToken<Confirmation>| dispatch.call(Selection::Band(answered));
    let (full, summary) = sidebar_summary(count, has_unchecked, all_ready);
    let tooltip = if !all_ready || has_unchecked {
        format!("Trash: {summary}. Click to see what is known.")
    } else if count == 0 {
        "Trash: empty. When you delete the last session using a managed checkout, its folder moves here instead of being deleted.".into()
    } else {
        format!(
            "Trash: {summary}. A managed checkout moves here when the last session using it is deleted. Click to see them, or to delete them for good."
        )
    };
    let accessible_name = format!("Checkout trash, {summary}");
    // Measuring disables confirmation, but retained known sizes still belong
    // in the total. Selection eligibility must not silently turn it into zero.
    let total_bytes = current_hosts
        .iter()
        .filter(|host| hosts_known && connected(host))
        .filter_map(|host| {
            state
                .read()
                .get(&host.id)
                .filter(|entry| entry.incarnation == host.incarnation)
                .and_then(|entry| entry.reply.as_ref())
                .and_then(|reply| reply.as_ref().ok())
                .map(|listing| {
                    listing
                        .checkouts
                        .iter()
                        .filter_map(|entry| entry.bytes)
                        .fold(0, u64::saturating_add)
                })
        })
        .fold(0, u64::saturating_add);
    let total_partial = !hosts_known
        || current_hosts
            .iter()
            .filter(|host| connected(host))
            .any(|host| {
                state
                    .read()
                    .get(&host.id)
                    .is_none_or(|entry| entry.loading || reply_partial(entry.reply.as_ref()))
            });
    rsx! {
        button {
            class: if full { "btn btn-neutral checkout-trash-button full" } else { "btn btn-neutral checkout-trash-button" },
            r#type: "button", aria_label: accessible_name, aria_haspopup: "dialog", "data-count": "{count}",
            "data-tooltip": tooltip, disabled: gate.busy(),
            onclick: move |_| { if !gate.busy_now() { notices.write().clear(); confirmation.clear(); open.set(true); } },
            span { class: if busy { "trash-delete-icon deleting" } else { "trash-delete-icon" }, TrashIcon { full } }
            if let Some(text) = count_text { span { class: "checkout-trash-count", "{text}" } }
        }
        if open() {
            div { class: "host-settings-backdrop", role: "presentation",
                ModalDialog { class: "checkout-trash-dialog", role: "dialog", aria_modal: "true", aria_labelledby: "checkout-trash-title", tabindex: "-1",
                    handle: DialogHandle::new(DIALOG),
                    focus: DialogFocus::remembered(".trash-close", Some(".trash-confirm-cancel, .trash-close"), false, Some(".trash-confirm-submit")),
                    on_escape: { let mut close = close.clone(); move |event: KeyboardEvent| { if !event.is_composing() { event.prevent_default(); event.stop_propagation(); close(); } } },
                    // Trash deliberately keeps its global controls with its
                    // totals. Shared heading styling must not cost archive
                    // height or move those controls into a new footer.
                    div { class: "dialog-header checkout-trash-header",
                        h2 { id: "checkout-trash-title", class: "dialog-title", span { class: if busy { "trash-delete-icon deleting" } else { "trash-delete-icon" }, TrashIcon { full } } "Trash" }
                        span { class: "checkout-trash-total", "{count} {checkouts(count)} · {space(total_bytes, total_partial)} on reachable hosts" }
                        div { class: "checkout-trash-header-actions",
                            button { class: "btn btn-danger trash-delete-all", disabled: busy || !all_ready || targets.is_empty(), onclick: move |_| { if deletion.peek().is_none() { confirmation.open(Confirmation { host: None, targets: targets.clone() }, ()); } }, "Delete all" }
                            // Native disabled controls suppress hover in some
                            // engines. The wrapper receives the explanation's
                            // pointer events while Close stays truly disabled.
                            span { class: "trash-close-help", "data-tooltip": busy.then_some("Available when deletion finishes."),
                                button { class: "btn btn-neutral trash-close", disabled: busy, onclick: { let mut close = close.clone(); move |_| close() }, "Close" }
                            }
                        }
                        // The status region stays mounted while idle so that a
                        // starting delete is a text change screen readers
                        // announce; a region inserted with its text already in
                        // place is announced unreliably, notably under WebKit.
                        div { class: if progress.is_some() { "checkout-trash-progress" } else { "checkout-trash-progress idle" }, role: "status",
                            if let Some(run) = &progress {
                                "{run.status()}"
                            }
                        }
                    }
                    ScrollRegion { class: "checkout-trash-body",
                    handle: DialogHandle::new(DIALOG), selector: ".checkout-trash-body", name: "Archived checkouts",
                    recovery: RecoveryFocus::new(".trash-close", None, true, RecoveryDelay::OneFrame),
                    if let Some(shown) = shown.as_ref().filter(|shown| shown.key().host.is_none()).cloned() {
                        ConfirmBand { shown, busy, submit_disabled: busy || gate.busy(), oncancel: band_cancel, onsubmit: band_submit }
                    }
                    p { "Folders of managed checkouts whose last session was deleted. They stay here, untouched, until you delete them. To get something back, copy it out of the folder shown under its host." }
                    if let Some(error) = &host_error { p { class: "checkout-trash-issue", "Host reachability could not be refreshed. Cleanup is disabled until it can be checked: " span { class: "peer-value", dir: "ltr", "{display_peer(error)}" } } }
                    if hosts_known && count == 0 && current_hosts.iter().filter(|host| connected(host)).all(|host| state.read().get(&host.id).is_some_and(|entry| !entry.loading && matches!(&entry.reply, Some(Ok(reply)) if reply.issues.is_empty()))) {
                        p { class: "checkout-trash-empty", "The trash is empty on reachable hosts." }
                    }
                    for host in current_hosts.iter() {
                        {
                            let states = state.read();
                            let entry = states.get(&host.id).filter(|entry| entry.incarnation == host.incarnation);
                            let listing = entry.and_then(|entry| entry.reply.as_ref()).and_then(|reply| reply.as_ref().ok());
                            let host_notices = notices.read().get(&host.id).cloned().unwrap_or_default();
                            let pending = entry.is_none_or(|entry| entry.loading);
                            let host_target = if hosts_known { target(host, entry) } else { None };
                            let action_host = host.clone();
                            let host_progress = progress.as_ref().and_then(|run| run.hosts.get(&host.id));
                            let deleting = host_progress.is_some();
                            let show = deleting || !connected(host) || pending || listing.is_none_or(|listing| !listing.checkouts.is_empty() || !listing.issues.is_empty()) || !host_notices.is_empty();
                            let name = host.name.escaped();
                            let totals = listing.map(listing_totals);
                            let host_count = totals.as_ref().map_or_else(|| "Count unknown".into(), ListingTotals::count_label);
                            let host_size = if connected(host) && hosts_known && pending { "Measuring…".into() } else { totals.as_ref().map_or_else(|| "Size unknown".into(), |totals| space(totals.bytes, totals.partial)) };
                            let paths = listing.map(|listing| listing.checkouts.iter().map(|entry| &entry.path)
                                .chain(listing.issues.iter().map(|issue| &issue.path))
                                .filter_map(|path| std::path::Path::new(path).parent().map(|path| path.display().to_string()))
                                .collect::<std::collections::BTreeSet<_>>()).unwrap_or_default();
                            rsx! {
                                if show {
                                    section { key: "{host.id}", class: if connected(host) { "checkout-trash-host" } else { "checkout-trash-host unreachable" }, "data-host-id": "{host.id}",
                                        div { class: "checkout-trash-host-header",
                                            div { strong { class: "peer-value", dir: "ltr", "{name}" } div { "{host_count} · {host_size}" }
                                                if let Some(run) = host_progress { div { class: "trash-host-progress", "{run.deleted} of {run.positions.len()} deleted" } }
                                            }
                                            button { class: "btn btn-danger btn-outline trash-host-delete", disabled: busy || host_target.is_none(),
                                                onclick: move |_| {
                                                    // Reads the click-time listing only to
                                                    // open the band. The band then shows,
                                                    // and its answer deletes, what this froze.
                                                    if deletion.peek().is_none()
                                                        && hosts.peek().refresh_error().is_none()
                                                        && let Some(target) = target(&action_host, state.peek().get(&action_host.id))
                                                    {
                                                        confirmation.open(Confirmation { host: Some(action_host.id), targets: vec![target] }, ());
                                                    }
                                                },
                                                "Delete all on " span { class: "peer-value", dir: "ltr", "{name}" }
                                            }
                                        }
                                        if let Some(shown) = shown.as_ref().filter(|shown| shown.key().host == Some(host.id)).cloned() {
                                            ConfirmBand { shown, busy, submit_disabled: busy || gate.busy(), oncancel: band_cancel, onsubmit: band_submit }
                                        }
                                        if !connected(host) { p { "Not reachable, so they can't be checked or deleted" } }
                                        if let Some(Err(error)) = entry.and_then(|entry| entry.reply.as_ref()) { div { class: "checkout-trash-issue peer-value", dir: "ltr", "{display_peer(error)}" } }
                                        if let Some(listing) = listing {
                                            if selection_size(listing) > MAX_SELECTION { p { "This host exceeds the 10,000-entry cleanup limit. Cleanup is unavailable for this listing." } }
                                            for checkout in listing.checkouts.iter() {
                                                {
                                                let entry_host = host.id;
                                                let entry_id = checkout.id.clone();
                                                let delete_label = format!("Delete {} permanently", display_peer(&checkout.name));
                                                let selected = host_progress.and_then(|run| run.positions.get(&checkout.id).map(|index| (run, *index)))
                                                    .filter(|(run, index)| !run.finished && *index >= run.next);
                                                rsx! {
                                                div { key: "{checkout.id}", class: "checkout-trash-entry",
                                                    div { class: "checkout-trash-entry-heading", strong { class: "peer-value font-mono", dir: "ltr", "{display_peer(&checkout.name)}" }
                                                        if !pending { if let Some(bytes) = checkout.bytes { span { "{space(bytes, false)}" } } }
                                                        if let Some((run, index)) = selected {
                                                            if let Some(started_at) = run.started_at.filter(|_| index == run.next) {
                                                                EntryProgress { started_at }
                                                            } else { span { class: "trash-entry-waiting", "Waiting" } }
                                                        }
                                                        button { class: "btn btn-danger btn-outline trash-entry-delete", aria_label: delete_label,
                                                            // An entry delete has no band of its own: keep it
                                                            // out of reach while a band asks about something
                                                            // else, and while moved rows settle (`SETTLE_MS`).
                                                            disabled: busy || gate.busy() || host_target.is_none() || shown.is_some() || !settled(),
                                                            onclick: move |_| {
                                                                if !*settled.peek() || confirmation.peek_key().is_some() { return; }
                                                                // Read the click-time host instead of granting a stale
                                                                // rendered row authority after a reconnect or refresh error.
                                                                let selected = {
                                                                    let hosts = hosts.peek();
                                                                    if hosts.refresh_error().is_some() { None }
                                                                    else { hosts.hosts().unwrap_or_default().iter()
                                                                        .find(|host| host.id == entry_host)
                                                                        .and_then(|host| entry_target(host, state.peek().get(&entry_host), &entry_id)) }
                                                                };
                                                                if let Some(selected) = selected { dispatch.call(Selection::Entry(selected)); }
                                                            }, "Delete"
                                                        }
                                                    }
                                                    div { class: "checkout-trash-entry-meta", BranchIcon {} span { class: "peer-value", dir: "ltr", "{display_peer(&checkout.repository)}" } span { {archived_when(checkout.archived_at)} } }
                                                }
                                                }
                                                }
                                            }
                                            for (index, issue) in listing.issues.iter().enumerate() { Issue { key: "listed-{index}", issue: issue.clone() } }
                                        }
                                        for (index, issue) in host_notices.iter().enumerate() { Issue { key: "outcome-{index}", issue: issue.clone() } }
                                        for path in paths { div { class: "checkout-trash-path", "On disk: " span { class: "peer-value", dir: "ltr", "{display_peer(&path)}" } } }
                                    }
                                }
                            }
                        }
                    }
                    }
                }
            }
        }
    }
}

/// Capture the row before its confirmed reply removes it. Geometry belongs to
/// the persistent button's DOM, so unmounting the list cannot leak a global map.
pub(super) fn capture_row(id: &str) {
    let id = serde_json::to_string(id).expect("string literal");
    document::eval(&format!(
        r#"(() => {{
        const id = {id}, button = document.querySelector('.checkout-trash-button');
        const row = Array.from(document.querySelectorAll('.session-row')).find(row => row.dataset.sessionId === id);
        if (!button || !row) return;
        (button.__farhelmTrashOrigins ??= new Map()).set(id, row.getBoundingClientRect());
    }})()"#
    ));
}

/// Only this client's affirmative archive reply plays the cue. A bare Delete,
/// a remote disappearance and reduced motion all skip flight and wiggle.
///
/// The flight outline is a terminal link cover (`data-link-cover`, see
/// app.css's z-index ladder). It only travels within the sidebar column, but a
/// link's destination preview is clamped to the window's right edge and can
/// reach left over the sidebar, where the outline draws above it. It is the
/// one cover exempt from the rule that a link's refusal explanation draws
/// above every cover: it lasts 650 ms, has no fill, and outranking its 10000
/// would put a terminal status line above everything in the app.
pub(super) fn finish_row_cue(id: &str, archived: bool) {
    let id = serde_json::to_string(id).expect("string literal");
    document::eval(&format!(
        r#"(() => {{
        const button = document.querySelector('.checkout-trash-button');
        if (!button) return;
        const origin = button.__farhelmTrashOrigins?.get({id});
        button.__farhelmTrashOrigins?.delete({id});
        if (!{archived} || matchMedia('(prefers-reduced-motion: reduce)').matches) return;
        const wiggle = () => {{ if (button.isConnected) button.animate([
            {{transform:'rotate(0deg)'}}, {{transform:'rotate(-16deg)'}}, {{transform:'rotate(12deg)'}},
            {{transform:'rotate(-8deg)'}}, {{transform:'rotate(4deg)'}}, {{transform:'rotate(0deg)'}}
        ], {{duration:900, easing:'ease-out'}}); }};
        if (!origin || origin.width <= 0 || origin.height <= 0) {{ wiggle(); return; }}
        const target = button.getBoundingClientRect();
        const outline = document.createElement('div');
        outline.className = 'checkout-trash-flight'; outline.setAttribute('aria-hidden','true');
        outline.setAttribute('data-link-cover','true');
        Object.assign(outline.style, {{left:origin.x+'px', top:origin.y+'px', width:origin.width+'px', height:origin.height+'px'}});
        document.body.append(outline);
        const x = target.x + target.width/2 - origin.x - origin.width/2;
        const y = target.y + target.height/2 - origin.y - origin.height/2;
        const flight = outline.animate([{{transform:'translate(0,0) scale(1)', opacity:.65}},
            {{transform:`translate(${{x}}px,${{y}}px) scale(.08)`, opacity:0}}], {{duration:650, easing:'ease-in'}});
        flight.onfinish = () => {{ outline.remove(); wiggle(); }};
        flight.oncancel = () => outline.remove();
    }})()"#
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::ArchivedCheckout;

    /// The optional text is also the rendered span, including its absence at
    /// zero. Only a changed decimal column count can add header width; counts
    /// with the same number of digits must not acquire a movement policy.
    #[farhelm_testtrace::test]
    fn sidebar_count_span_preserves_text_and_only_changes_width_at_digit_boundaries() {
        let cases = [
            (0, None, 0),
            (1, Some("1"), 1),
            (2, Some("2"), 1),
            (9, Some("9"), 1),
            (10, Some("10"), 2),
            (11, Some("11"), 2),
        ];
        for (count, text, columns) in cases {
            let shown = sidebar_count_text(count);
            assert_eq!(shown.as_deref(), text);
            assert_eq!(shown.as_ref().map_or(0, String::len), columns);
        }
    }

    /// Build a connected host snapshot with a nonzero incarnation, so the
    /// selection proof exercises authority rather than an offline refusal.
    fn host() -> Host {
        serde_json::from_value(serde_json::json!({
            "id": 1, "kind": "local", "name": "local", "identity": null,
            "remote_farhelm": null, "remote_state_dir": null, "incarnation": 7,
            "state": { "phase": "connected", "identity": null,
                "build_version": "0.0.0", "refresh": { "status": "ok", "sessions": 0 } },
        }))
        .expect("connected host fixture")
    }

    std::thread_local! {
        /// The band slot [`band_app`] mounted, handed out to the test body.
        static BAND: std::cell::Cell<Option<ConfirmSlot<Confirmation>>> =
            const { std::cell::Cell::new(None) };
    }

    /// Mount the dialog's confirmation slot the way `TrashControl` creates
    /// it, so a test can open, answer and cancel band openings directly.
    fn band_app() -> Element {
        let slot = use_confirm_slot::<Confirmation, ()>();
        BAND.with(|cell| cell.set(Some(slot)));
        rsx! {}
    }

    /// A one-archive host selection, the shape a per-host band freezes.
    fn selected() -> Target {
        Target {
            host: 1,
            incarnation: 7,
            name: "first host".into(),
            ids: vec!["archive-a".into()],
            count: 1,
            unchecked: 0,
            bytes: 4096,
            partial: false,
        }
    }

    /// Spec (`Selection::targets`, `Selection::commit`): a band answer
    /// deletes the targets of the opening its render drew, and only while
    /// that opening is the one on screen. Once the band is cancelled or
    /// replaced (by another host's band, the global band, the same host with
    /// other IDs or a new connection, or a band with identical content opened
    /// again) the answer yields nothing and leaves the band now on screen
    /// open. Starting the run consumes the confirmed opening, so a second
    /// answer finds nothing.
    ///
    /// Why: confirming deletes archived checkouts permanently. A stale
    /// affirmative answer queued in the same event burst as a cancel and a
    /// reopen must never borrow the authority of a band the user has not
    /// read yet, even one whose text happens to match.
    #[farhelm_testtrace::test]
    fn confirmation_answer_deletes_only_its_unchanged_selection() {
        let mut dom = VirtualDom::new(band_app);
        dom.rebuild_in_place();
        let mut slot = BAND.with(std::cell::Cell::get).expect("the slot mounted");

        let selected = selected();
        let answered = Confirmation {
            host: Some(selected.host),
            targets: vec![selected.clone()],
        };
        let mut other_host = answered.clone();
        other_host.host = Some(2);
        other_host.targets[0].host = 2;
        other_host.targets[0].name = "second host".into();
        let global = Confirmation {
            host: None,
            targets: vec![selected.clone(), other_host.targets[0].clone()],
        };
        let mut other_ids = answered.clone();
        other_ids.targets[0].ids = vec!["archive-b".into()];
        let mut reconnected = answered.clone();
        reconnected.targets[0].incarnation += 1;

        dom.in_runtime(|| {
            let shown = slot.open(answered.clone(), ());
            let answer = Selection::Band(shown.clone());
            assert!(answer.targets(&slot) == Some(vec![selected.clone()]));

            // Cancelled, then answered.
            slot.cancel_for(&shown);
            assert!(answer.targets(&slot).is_none());

            // Replaced by every kind of other band, the identical one included.
            for replacement in [other_host, global.clone(), other_ids, reconnected, answered] {
                let current = slot.open(replacement.clone(), ());
                assert!(
                    answer.targets(&slot).is_none(),
                    "a stale answer selects nothing"
                );
                assert!(!answer.commit(&mut slot), "and cannot consume the band");
                assert!(slot.is_current(&current), "the band on screen stays open");
                let fresh = Selection::Band(current);
                assert!(fresh.targets(&slot) == Some(replacement.targets));
            }

            let current = slot.open(global.clone(), ());
            let answer = Selection::Band(current);
            assert!(answer.targets(&slot) == Some(global.targets.clone()));
            assert!(answer.commit(&mut slot), "the run consumes its band");
            assert!(!slot.is_open());
            assert!(
                answer.targets(&slot).is_none(),
                "a second answer finds nothing"
            );
            assert!(!answer.commit(&mut slot));
        });
    }

    /// Spec (`Selection::targets`, `cancel_confirmation`): the answer deletes
    /// the band's frozen targets even after the host's listing gains an
    /// archive; an entry delete deletes its one checkout regardless of the
    /// band and closes whatever band is open as its run starts; a stale
    /// Cancel leaves the band now on screen open, and a current one closes it.
    ///
    /// Why: the band's text is what the user agreed to. Recomputing the
    /// selection when the click runs would delete an archive the band never
    /// mentioned, and a Cancel drawn for an earlier band must not dismiss a
    /// question the user has not yet seen.
    #[farhelm_testtrace::test]
    fn band_answers_act_on_what_the_band_showed() {
        let mut dom = VirtualDom::new(band_app);
        dom.rebuild_in_place();
        let mut slot = BAND.with(std::cell::Cell::get).expect("the slot mounted");

        let host = host();
        let archive = |id: &str| ArchivedCheckout {
            id: id.into(),
            name: id.into(),
            repository: "fixture/repo".into(),
            path: format!("/work/trash/{id}"),
            archived_at: None,
            bytes: Some(4096),
        };
        let mut state = HostTrash {
            incarnation: host.incarnation,
            loading: false,
            reply: Some(Ok(CheckoutTrashListing {
                checkouts: vec![archive("shown")],
                issues: vec![],
            })),
        };

        dom.in_runtime(|| {
            let frozen = target(&host, Some(&state)).expect("ready selection");
            let shown = slot.open(
                Confirmation {
                    host: Some(host.id),
                    targets: vec![frozen],
                },
                (),
            );
            state
                .reply
                .as_mut()
                .unwrap()
                .as_mut()
                .unwrap()
                .checkouts
                .push(archive("later"));
            assert_eq!(target(&host, Some(&state)).unwrap().ids, ["shown", "later"]);
            let answered = Selection::Band(shown.clone())
                .targets(&slot)
                .expect("the band is still on screen");
            assert_eq!(
                answered[0].ids,
                ["shown"],
                "the band's targets, not the live listing's"
            );

            let entry = Selection::Entry(
                entry_target(&host, Some(&state), "later").expect("eligible checkout"),
            );
            assert_eq!(entry.targets(&slot).unwrap()[0].ids, ["later"]);
            assert!(entry.commit(&mut slot), "an entry run always starts");
            assert!(!slot.is_open(), "and closes the open band");

            // `cancel_confirmation` with a host band moves no focus, so it
            // runs here without a document.
            let reopened = slot.open(shown.key().clone(), ());
            cancel_confirmation(slot, &shown);
            assert!(slot.is_current(&reopened), "a stale Cancel leaves the band");
            cancel_confirmation(slot, &reopened);
            assert!(!slot.is_open(), "the current Cancel closes it");
        });
    }

    /// Spec (`close_action`): a Close or Escape whose render showed a band
    /// dismisses that band, whatever is open now; one whose render showed no
    /// band closes the dialog while no band is open, and does nothing once a
    /// band has opened since.
    ///
    /// Why: Close and Escape are drawn for one state of the dialog and can
    /// arrive in the same event burst as a click that opens a band. Closing
    /// the dialog then would hide a question the user has not read, and
    /// dismissing it would answer a band the Close was never drawn for.
    #[farhelm_testtrace::test]
    fn close_and_escape_act_on_what_their_render_showed() {
        let mut dom = VirtualDom::new(band_app);
        dom.rebuild_in_place();
        let mut slot = BAND.with(std::cell::Cell::get).expect("the slot mounted");

        dom.in_runtime(|| {
            let shown = slot.open(
                Confirmation {
                    host: Some(1),
                    targets: vec![selected()],
                },
                (),
            );
            for band_open_now in [true, false] {
                assert!(
                    matches!(close_action(Some(&shown), band_open_now), CloseAction::Dismiss(token) if *token == shown),
                    "a shown band is the one dismissed"
                );
            }
            assert!(
                matches!(close_action(None, false), CloseAction::CloseDialog),
                "with no band shown or open, the dialog closes"
            );
            assert!(
                matches!(close_action(None, true), CloseAction::Nothing),
                "a band opened since the render stays, and so does the dialog"
            );
        });
    }

    /// Host summaries must not present uninspectable archives as an exact zero.
    /// Duplicate diagnostics describe one unchecked entry, and a mixed listing
    /// keeps its measured bytes while disclosing the missing part. The same
    /// evidence must reach the frozen confirmation without widening its IDs.
    /// The sidebar must also distinguish unchecked-only trash from empty trash.
    #[farhelm_testtrace::test]
    fn host_totals_qualify_unchecked_entries_and_partial_sizes() {
        let host = host();
        assert!(connected(&host));
        let issue = CheckoutTrashIssue {
            id: "unchecked".into(),
            path: "/work/trash/unreadable".into(),
            message: "could not check ownership".into(),
        };
        let checkout = |bytes| ArchivedCheckout {
            id: "known".into(),
            name: "old".into(),
            repository: "fixture/repo".into(),
            path: "/work/trash/old".into(),
            archived_at: None,
            bytes,
        };
        for (listing, expected_count, expected_size, expected_unchecked, expected_sidebar) in [
            (
                CheckoutTrashListing {
                    checkouts: vec![],
                    issues: vec![issue.clone(), issue.clone()],
                },
                "0 checkouts · 1 unchecked entry",
                "0 B measured · partial",
                1,
                "0 known archived checkouts, some entries or hosts unchecked",
            ),
            (
                CheckoutTrashListing {
                    checkouts: vec![checkout(Some(4096))],
                    issues: vec![
                        issue.clone(),
                        CheckoutTrashIssue {
                            id: "another".into(),
                            ..issue.clone()
                        },
                    ],
                },
                "1 checkout · 2 unchecked entries",
                "4.0 KiB measured · partial",
                2,
                "1 known archived checkout, some entries or hosts unchecked",
            ),
            (
                CheckoutTrashListing {
                    checkouts: vec![checkout(None)],
                    issues: vec![],
                },
                "1 checkout",
                "0 B measured · partial",
                0,
                "1 archived checkout",
            ),
            (
                CheckoutTrashListing {
                    checkouts: vec![checkout(Some(4096))],
                    issues: vec![],
                },
                "1 checkout",
                "4.0 KiB",
                0,
                "1 archived checkout",
            ),
        ] {
            let totals = listing_totals(&listing);
            assert_eq!(totals.count_label(), expected_count);
            assert_eq!(space(totals.bytes, totals.partial), expected_size);
            assert_eq!(totals.unchecked, expected_unchecked);
            assert_eq!(
                sidebar_summary(totals.count, totals.unchecked > 0, true),
                (true, expected_sidebar.into())
            );
            let state = HostTrash {
                incarnation: host.incarnation,
                loading: false,
                reply: Some(Ok(listing)),
            };
            let selected = target(&host, Some(&state)).expect("settled nonempty listing");
            assert_eq!(selected.unchecked, expected_unchecked);
            assert_eq!(selected.ids.len(), selected.count + expected_unchecked);
            assert_eq!(space(selected.bytes, selected.partial), expected_size);
        }
        assert_eq!(sidebar_summary(0, false, true), (false, "empty".into()));
        assert_eq!(
            sidebar_summary(0, false, false),
            (
                false,
                "0 known archived checkouts, some entries or hosts unchecked".into()
            )
        );
    }

    /// The cleanup batch cap must not hide unknown bytes in a successful listing.
    /// This fits within a control reply but exceeds the unique-ID deletion limit;
    /// measured bytes stay visible and partial independently of cleanup eligibility.
    #[farhelm_testtrace::test]
    fn over_limit_listing_keeps_global_measurement_partial() {
        let host = host();
        assert!(connected(&host));
        let mut listing = CheckoutTrashListing {
            checkouts: (0..=MAX_SELECTION)
                .map(|id| ArchivedCheckout {
                    id: id.to_string(),
                    name: "old".into(),
                    repository: "fixture/repo".into(),
                    path: "/work/trash/old".into(),
                    archived_at: None,
                    bytes: Some(0),
                })
                .collect(),
            issues: vec![],
        };
        listing.checkouts[0].bytes = Some(4096);
        listing.checkouts[1].bytes = None;
        assert!(
            serde_json::to_vec(&listing).unwrap().len() < farhelm_proto::MAX_FRAME_LEN as usize / 2
        );
        assert!(selection_size(&listing) > MAX_SELECTION);
        let mut state = HostTrash {
            incarnation: host.incarnation,
            loading: false,
            reply: Some(Ok(listing)),
        };
        assert!(target(&host, Some(&state)).is_none());
        assert!(reply_partial(state.reply.as_ref()));
        let totals = listing_totals(state.reply.as_ref().unwrap().as_ref().unwrap());
        assert_eq!(
            space(totals.bytes, reply_partial(state.reply.as_ref())),
            "4.0 KiB measured · partial"
        );
        // An over-limit selection alone does not make a complete measurement partial.
        state.reply.as_mut().unwrap().as_mut().unwrap().checkouts[1].bytes = Some(0);
        assert!(target(&host, Some(&state)).is_none());
        assert!(!reply_partial(state.reply.as_ref()));
        assert!(reply_partial(None));
        assert!(reply_partial(Some(&Err("unavailable".into()))));
    }

    /// A row click must select only its checkout, never a neighbouring archive
    /// or diagnostic. Requiring the complete host's eligibility prevents a
    /// single-ID request from bypassing stale evidence or the batch-size limit.
    #[farhelm_testtrace::test]
    fn entry_cleanup_selects_one_checkout_with_current_host_authority() {
        let host = host();
        assert!(connected(&host));
        let checkout = |id: &str| ArchivedCheckout {
            id: id.into(),
            name: id.into(),
            repository: "fixture/repo".into(),
            path: format!("/work/trash/{id}"),
            archived_at: None,
            bytes: None,
        };
        let mut state = HostTrash {
            incarnation: host.incarnation,
            loading: false,
            reply: Some(Ok(CheckoutTrashListing {
                checkouts: vec![checkout("selected"), checkout("neighbour")],
                issues: vec![CheckoutTrashIssue {
                    id: "issue".into(),
                    path: String::new(),
                    message: "unverified".into(),
                }],
            })),
        };
        assert_eq!(target(&host, Some(&state)).unwrap().ids.len(), 3);
        let selected = entry_target(&host, Some(&state), "selected").expect("eligible checkout");
        assert_eq!(selected.ids, ["selected"]);
        assert_eq!(
            (selected.host, selected.incarnation),
            (host.id, host.incarnation)
        );
        assert!(entry_target(&host, Some(&state), "issue").is_none());
        assert!(entry_target(&host, Some(&state), "absent").is_none());
        state.loading = true;
        assert!(entry_target(&host, Some(&state), "selected").is_none());
        state.loading = false;
        state.incarnation += 1;
        assert!(entry_target(&host, Some(&state), "selected").is_none());
        state.incarnation = host.incarnation;
        let listing = state.reply.as_mut().unwrap().as_mut().unwrap();
        listing
            .checkouts
            .extend((0..MAX_SELECTION).map(|i| checkout(&format!("extra-{i}"))));
        assert!(selection_size(listing) > MAX_SELECTION);
        assert!(entry_target(&host, Some(&state), "selected").is_none());
        assert_eq!(
            selected.ids,
            ["selected"],
            "the selection is a value copy, so later listings leave it as clicked"
        );
    }

    /// Duplicate messages must not widen a destructive selection, and a
    /// confirmation already shown must not acquire an archive arriving later.
    #[farhelm_testtrace::test]
    fn cleanup_selection_deduplicates_issues_and_freezes_archives() {
        let host = host();
        assert!(connected(&host));
        let issue = CheckoutTrashIssue {
            id: "unchecked".into(),
            path: "/work/trash/old".into(),
            message: "preserved".into(),
        };
        let mut state = HostTrash {
            incarnation: host.incarnation,
            loading: false,
            reply: Some(Ok(CheckoutTrashListing {
                checkouts: vec![ArchivedCheckout {
                    id: "known".into(),
                    name: "old".into(),
                    repository: "fixture/repo".into(),
                    path: "/work/trash/old".into(),
                    archived_at: None,
                    bytes: None,
                }],
                issues: vec![issue.clone(), issue],
            })),
        };
        let frozen = target(&host, Some(&state)).expect("ready selection");
        assert_eq!(frozen.ids, ["known", "unchecked"]);
        assert_eq!(
            (frozen.count, frozen.unchecked, frozen.partial),
            (1, 1, true)
        );
        state
            .reply
            .as_mut()
            .unwrap()
            .as_mut()
            .unwrap()
            .checkouts
            .push(ArchivedCheckout {
                id: "later".into(),
                name: "later".into(),
                repository: "fixture/repo".into(),
                path: "/work/trash/later".into(),
                archived_at: None,
                bytes: Some(4096),
            });
        assert_eq!(frozen.ids, ["known", "unchecked"]);
        assert_eq!(target(&host, Some(&state)).unwrap().ids.len(), 3);
        state.loading = true;
        assert!(target(&host, Some(&state)).is_none());
        state.loading = false;
        state.incarnation += 1;
        assert!(target(&host, Some(&state)).is_none());
    }
}

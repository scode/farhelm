//! Show archives and accepted cleanup without treating a receipt as reclaimed space.
//!
//! Idle counts are event-driven; hosts actively freeing space are polled until
//! their next successful listing says removal has ended. Opening the dialog asks for bounded disk usage;
//! confirmations and immediate single deletes freeze IDs and connection incarnations, while the supervisor
//! rechecks ownership before any mutation. Read generations prevent an earlier
//! count/measurement from restoring entries removed by this client's cleanup.
//! A capable host retains queued, running and stuck entries; explicit Retry
//! shares the existing deletion owner rather than starting another cleanup path.
//! The list gains a named tab stop when none of its actions can take focus,
//! so keyboard users can still scroll retained host information in WebKit
//! while a read or deletion is pending.

use crate::anchored_popovers::{AnchorRegion, MovementOwner, use_rendered_movement};
use crate::hosts::HostsRead;
use crate::icons::{BranchIcon, TrashIcon};
use crate::modal_dialog::{
    DialogActions, DialogFocus, DialogFooter, DialogHandle, DialogHeader, ModalDialog,
    RecoveryDelay, RecoveryFocus, ScrollRegion,
};
use crate::ops::{ConfirmSlot, ConfirmToken, PaneGate, use_confirm_slot};
use crate::peer::display_peer;
use crate::{ApiBase, Host, HostId, HostPhase};
use dioxus::prelude::*;
use farhelm_proto::{CheckoutTrashIssue, CheckoutTrashListing};
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

const DIALOG: &str = ".checkout-trash-dialog";
/// Only whole-dialog dismissal returns here; cancelling a confirmation stays inside.
/// A retained mark can become visible during cleanup, concealing the old trigger.
const RETURN_FOCUS: &str = "(document.querySelector('.marked-menu-toggle') ?? document.querySelector('.checkout-trash-button'))?.focus({preventScroll:true});";
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
/// A host's running report or capable cleanup receipt starts this cadence.
/// A failed read preserves progress, so a temporary outage cannot imply completion.
const REMOVAL_POLL_MS: u64 = 3_000;

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
    /// The last same-incarnation progress report or accepted cleanup awaiting
    /// confirmation. Read failures retain it until a successful list settles it.
    removing: bool,
    /// A host-local request order: polling one host cannot supersede another's read.
    read_serial: u64,
}

/// A lost reply belongs to its frozen selection, not every later request to
/// that host. Keep the unresolved IDs privately while displaying the existing
/// host-level issue; unrelated successes must not acknowledge that uncertainty.
#[derive(Clone)]
enum TrashNotice {
    Checkout(CheckoutTrashIssue),
    Uncertain {
        issue: CheckoutTrashIssue,
        unresolved: HashSet<String>,
    },
}

impl TrashNotice {
    /// Reuse the existing issue surface without exposing local bookkeeping.
    fn issue(&self) -> &CheckoutTrashIssue {
        match self {
            Self::Checkout(issue) | Self::Uncertain { issue, .. } => issue,
        }
    }

    /// Retain one uncertain outcome for exactly the IDs that were submitted.
    /// A connection-change refusal uses the same lifetime even when no request
    /// was sent: only a later outcome for those IDs can replace its explanation.
    fn uncertain(ids: &[String], message: String) -> Self {
        Self::Uncertain {
            issue: CheckoutTrashIssue {
                id: String::new(),
                path: String::new(),
                message,
            },
            unresolved: ids.iter().cloned().collect(),
        }
    }

    /// A settled outcome resolves only the IDs it actually reports. Partial
    /// overlap shrinks an old uncertain selection rather than discarding it.
    fn resolve(&mut self, resolved: &HashSet<&str>) -> bool {
        match self {
            Self::Checkout(issue) => !resolved.contains(issue.id.as_str()),
            Self::Uncertain { unresolved, .. } => {
                unresolved.retain(|id| !resolved.contains(id.as_str()));
                !unresolved.is_empty()
            }
        }
    }

    /// A newer lost reply replaces uncertainty only for its overlapping IDs.
    /// A failure supplies no evidence to acknowledge an ID-specific refusal.
    fn replace_uncertainty(&mut self, ids: &[String]) -> bool {
        match self {
            Self::Checkout(_) => true,
            Self::Uncertain { unresolved, .. } => {
                unresolved.retain(|id| !ids.contains(id));
                !unresolved.is_empty()
            }
        }
    }
}

/// Only a successful registry snapshot can establish removal. Disconnection,
/// initial loading and a failed refresh preserve a host's inspectable notices.
fn host_removed(read: &HostsRead, host: HostId) -> bool {
    read.refresh_error().is_none()
        && read
            .hosts()
            .is_some_and(|hosts| hosts.iter().all(|entry| entry.id != host))
}

/// Apply one host read only while both its mutation fence and local request
/// order are current. Admission is synchronous even when the returned future
/// is spawned later: deletion buttons must reflect the new measurement before
/// another click can use the retained old listing. All event reads and removal
/// polls share this boundary, including rejection of late size-free replies.
fn read_host(
    base: String,
    host: HostId,
    incarnation: u64,
    measure: bool,
    mut state: Signal<HashMap<HostId, HostTrash>>,
    generation: Signal<u64>,
    current: u64,
) -> impl std::future::Future<Output = ()> {
    let serial = (*generation.peek() == current).then(|| {
        let mut states = state.write();
        let entry = states.entry(host).or_default();
        if entry.incarnation != incarnation {
            *entry = HostTrash {
                incarnation,
                ..HostTrash::default()
            };
        }
        entry.loading = true;
        entry.read_serial = entry.read_serial.wrapping_add(1);
        entry.read_serial
    });
    async move {
        let Some(serial) = serial else {
            return;
        };
        let reply = crate::api::fetch_checkout_trash(&base, host, incarnation, measure).await;
        if *generation.peek() != current {
            return;
        }
        if let Some(entry) = state
            .write()
            .get_mut(&host)
            .filter(|entry| entry.incarnation == incarnation && entry.read_serial == serial)
        {
            if let Ok(listing) = &reply {
                entry.removing = listing.removing;
            }
            entry.reply = Some(reply);
            entry.loading = false;
        }
    }
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
/// the band it is drawn in (a host's, or `None` for "Delete all" above the
/// pinned footer) and the targets frozen when it opened.
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
    /// Called after the run has checked the page, so a confirmation refused
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
/// keeps known checkout totals separate from unchecked record cleanup.
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
/// A queued or running entry has no second action; a failed one uses this same
/// authority boundary for its explicit Retry.
fn entry_target(host: &Host, state: Option<&HostTrash>, id: &str) -> Option<Target> {
    let mut selected = target(host, state)?;
    let listing = state?.reply.as_ref()?.as_ref().ok()?;
    let checkout = listing.checkouts.iter().find(|entry| entry.id == id)?;
    if checkout.removing {
        return None;
    }
    selected.ids = vec![checkout.id.clone()];
    selected.count = 1;
    selected.unchecked = 0;
    selected.bytes = checkout.bytes.unwrap_or_default();
    selected.partial = checkout.bytes.is_none();
    Some(selected)
}

/// Acceptance starts cleanup; only a later capable listing proves the files are gone.
///
/// Older supervisors hide accepted rows themselves. Their optimistic disappearance
/// stays unchanged, while a capable peer's retained row keeps the running indicator
/// and its existing completion poll alive through an explicitly requested Retry.
fn accepted_cleanup(state: &mut HostTrash, ids: &[String]) {
    let Some(Ok(listing)) = state.reply.as_mut() else {
        return;
    };
    if listing.reports_retired {
        // An accepted diagnostic-only ID has no row to mark. Keep the poll
        // alive for it too: a failed immediate refresh cannot prove that the
        // newly accepted work finished or that it never started.
        listing.removing |= !ids.is_empty();
        state.removing |= !ids.is_empty();
        for checkout in &mut listing.checkouts {
            if ids.contains(&checkout.id) {
                checkout.removing = true;
                checkout.stuck = None;
            }
        }
    } else {
        listing.checkouts.retain(|entry| !ids.contains(&entry.id));
    }
    listing.issues.retain(|entry| !ids.contains(&entry.id));
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
    let timestamp = utc_timestamp(seconds);
    rsx! { "Archived " span { class: "font-mono", "{timestamp}" } }
}

/// Render the host's recorded time without substituting a client or filesystem clock.
/// Archive dates and cleanup failures share this display but have distinct causes.
fn utc_timestamp(seconds: i64) -> String {
    let (year, month, day) = farhelm_proto::time::civil_from_days(seconds.div_euclid(86400));
    let time = seconds.rem_euclid(86400);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        time / 3600,
        time % 3600 / 60
    )
}

/// Dismiss the band opening `shown`, the one the cancelling render drew
/// (its Cancel button, or the dialog's Close and Escape while it was shown).
/// A stale cancel, drawn for a band since answered or replaced, does nothing,
/// focus included.
///
/// Cancelling a global choice returns to its pinned trigger, including Escape.
/// Removing the focused band alone would discard its Cancel button and leave
/// keyboard users without a useful next action.
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
        // A confirmation can name more hosts than fit in its scroll region:
        // the archive body for a host, or the bounded band above the footer.
        // Reveal the answers there; the warning remains reachable by scrolling
        // upwards, without moving the pinned header or footer.
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
/// Marking may conceal the trigger after a listing refresh, but never this
/// owner's active dialog or requests; trigger presentation is independent.
#[component]
pub(super) fn TrashControl(
    hosts: Signal<HostsRead>,
    revision: Signal<u64>,
    gate: PaneGate,
    #[props(default)] trigger_hidden: bool,
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
    let mut notices = use_signal(HashMap::<HostId, Vec<TrashNotice>>::new);
    // Trash admission is local to this owner. Accepted host requests outlive
    // the modal, without holding the two panes' shared operation claim.
    // Each pending host carries permission to retain notices. Healthy registry
    // removal revokes it for the rest of that request, even if a later registry
    // refresh fails. This fence ends with the request; no host history remains.
    let mut deletion = use_signal(HashMap::<HostId, bool>::new);
    let mut polling = use_signal(HashSet::<(HostId, u64)>::new);
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
        if hosts_known {
            // Reconcile notices even while deletes suspend filesystem reads.
            // Unreachable hosts remain in this authoritative registry signature.
            notices
                .write()
                .retain(|id, _| signature.iter().any(|(host, _, _)| host == id));
            let removed: Vec<_> = deletion
                .peek()
                .iter()
                .filter(|(id, retain)| {
                    **retain && !signature.iter().any(|(host, _, _)| host == *id)
                })
                .map(|(id, _)| *id)
                .collect();
            if !removed.is_empty() {
                let mut pending = deletion.write();
                for host in removed {
                    pending.insert(host, false);
                }
            }
        }
        if !deletion().is_empty() {
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
            let base = read_base.clone();
            spawn(read_host(
                base,
                host,
                incarnation,
                measure,
                state,
                generation,
                current,
            ));
        }
    });
    let poll_base = base.clone();
    use_effect(move || {
        let active: Vec<_> = state
            .read()
            .iter()
            .filter(|(_, entry)| entry.removing)
            .map(|(host, entry)| (*host, entry.incarnation))
            .collect();
        for (host, incarnation) in active {
            // This set owns timers, not read authority. A render or another
            // host's reply cannot create a second loop for the same connection.
            if !polling.write().insert((host, incarnation)) {
                continue;
            }
            let base = poll_base.clone();
            spawn(async move {
                loop {
                    crate::reader::sleep_ms(REMOVAL_POLL_MS).await;
                    let connected_now =
                        hosts
                            .peek()
                            .hosts()
                            .unwrap_or_default()
                            .iter()
                            .any(|entry| {
                                entry.id == host
                                    && connected(entry)
                                    && entry.incarnation == incarnation
                            });
                    let status = state
                        .peek()
                        .get(&host)
                        .filter(|entry| entry.incarnation == incarnation)
                        .map(|entry| (entry.removing, entry.loading));
                    let Some((true, loading)) = status.filter(|_| connected_now) else {
                        break;
                    };
                    if loading || !deletion.peek().is_empty() {
                        continue;
                    }
                    let current = *generation.peek();
                    let measure = *open.peek();
                    read_host(
                        base.clone(),
                        host,
                        incarnation,
                        measure,
                        state,
                        generation,
                        current,
                    )
                    .await;
                }
                polling.write().remove(&(host, incarnation));
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
    let pending_hosts = deletion();
    let busy = !pending_hosts.is_empty();
    let delete_base = base;
    // Both entry clicks and confirmed bulk choices cross the same authority,
    // operation-guard and completion boundary. Only selection/confirmation differ.
    let dispatch = use_callback(move |selection: Selection| {
        if !deletion.peek().is_empty() || gate.busy_now() {
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
        if !selection.commit(&mut confirmation) {
            return;
        }
        deletion.set(targets.iter().map(|target| (target.host, true)).collect());
        generation += 1; // supersede every pre-delete count and size reply
        if matches!(selection, Selection::Band(_)) {
            // The persistent owner keeps the requests alive after its modal
            // disappears. Returning focus belongs to submission, not completion.
            DialogHandle::new(DIALOG).close(RETURN_FOCUS);
            open.set(false);
        }
        for target in targets {
            let base = delete_base.clone();
            spawn(async move {
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
                    if deletion.peek().get(&target.host).copied().unwrap_or(false)
                        && !host_removed(&hosts.peek(), target.host)
                    {
                        let mut notices = notices.write();
                        let retained = notices.entry(target.host).or_default();
                        retained.retain_mut(|notice| notice.replace_uncertainty(&target.ids));
                        retained.push(TrashNotice::uncertain(
                            &target.ids,
                            "The host connection changed; cleanup was not sent. Rechecking the trash.".into(),
                        ));
                    }
                } else {
                    // One frozen batch per host. Hosts progress independently;
                    // neither a newer archive nor a later listing expands it.
                    let result = crate::api::empty_checkout_trash(
                        &base,
                        target.host,
                        target.incarnation,
                        &target.ids,
                    )
                    .await;
                    // Healthy removal revokes this request's notice authority.
                    // The pending flag also remembers a removal observed before
                    // a subsequent registry outage; checking only today's error
                    // would let this late reply recreate an orphaned warning.
                    if deletion.peek().get(&target.host).copied().unwrap_or(false)
                        && !host_removed(&hosts.peek(), target.host)
                    {
                        match result {
                            Ok(reply) => {
                                // The reply confirms accepted IDs, not reclaimed
                                // space. Retain capable rows until a fresh list;
                                // the request's own diagnostics stay visible below.
                                if let Some(entry) = state
                                    .write()
                                    .get_mut(&target.host)
                                    .filter(|entry| entry.incarnation == target.incarnation)
                                {
                                    accepted_cleanup(entry, &reply.removed);
                                }
                                let mut notices = notices.write();
                                let retained = notices.entry(target.host).or_default();
                                let resolved: HashSet<_> = reply
                                    .removed
                                    .iter()
                                    .map(String::as_str)
                                    .chain(reply.issues.iter().map(|issue| issue.id.as_str()))
                                    .filter(|id| {
                                        target.ids.iter().any(|selected| selected.as_str() == *id)
                                    })
                                    .collect();
                                retained.retain_mut(|notice| notice.resolve(&resolved));
                                retained
                                    .extend(reply.issues.into_iter().map(TrashNotice::Checkout));
                            }
                            Err(error) => {
                                let mut notices = notices.write();
                                let retained = notices.entry(target.host).or_default();
                                retained
                                    .retain_mut(|notice| notice.replace_uncertainty(&target.ids));
                                retained.push(TrashNotice::uncertain(&target.ids,
                                format!("The cleanup reply could not be confirmed; some folders may already be deleted. Rechecking the trash: {error}")));
                            }
                        }
                    }
                }
                deletion.write().remove(&target.host);
                if deletion.peek().is_empty() {
                    refresh += 1;
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
    // open band, the reachability error, pending requests, and per host its
    // state, entries (including failure details that change their height),
    // issues and notices), and re-arm the pause in the same
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
                listing
                    .checkouts
                    .iter()
                    .map(|c| {
                        (
                            &c.id,
                            c.removing,
                            c.stuck.as_ref().map(|stuck| (&stuck.reason, stuck.at)),
                        )
                    })
                    .collect::<Vec<_>>(),
                listing.issues.len(),
            )),
            _ => None,
        };
        let pending = entry.is_none_or(|entry| entry.loading);
        let notices = notices.read().get(&host.id).map(Vec::len);
        let deleting = pending_hosts.contains_key(&host.id);
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
        move || match close_action(shown.as_ref(), confirmation.peek_key().is_some()) {
            CloseAction::Dismiss(shown) => cancel_confirmation(confirmation, shown),
            CloseAction::Nothing => {}
            CloseAction::CloseDialog => {
                DialogHandle::new(DIALOG).close(RETURN_FOCUS);
                open.set(false);
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
    let mut tooltip = if !all_ready || has_unchecked {
        format!("Trash: {summary}. Click to see what is known.")
    } else if count == 0 {
        "Trash: empty. When you delete the last session using a managed checkout, its folder moves here instead of being deleted.".into()
    } else {
        format!(
            "Trash: {summary}. A managed checkout moves here when the last session using it is deleted. Click to see them, or to delete them for good."
        )
    };
    let removing_hosts = current_hosts
        .iter()
        .filter(|host| connected(host))
        .filter(|host| {
            pending_hosts.contains_key(&host.id)
                || state
                    .read()
                    .get(&host.id)
                    .is_some_and(|entry| entry.incarnation == host.incarnation && entry.removing)
        })
        .map(|host| host.name.escaped())
        .collect::<Vec<_>>();
    let emptying = !pending_hosts.is_empty() || !removing_hosts.is_empty();
    if emptying {
        let activity = if removing_hosts.is_empty() {
            "Emptying the trash.".into()
        } else {
            format!(
                "Emptying the trash: freeing space on {}.",
                removing_hosts.join(", ")
            )
        };
        tooltip = format!("{activity} {tooltip}");
    }
    if notices.read().values().any(|issues| !issues.is_empty()) {
        tooltip.push_str(" Some checkouts could not be deleted. Open the trash to see why.");
    }
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
            hidden: trigger_hidden,
            r#type: "button", aria_label: accessible_name, aria_haspopup: "dialog", "data-count": "{count}",
            "data-tooltip": tooltip, disabled: gate.busy(),
            onclick: move |_| { if !gate.busy_now() { confirmation.clear(); open.set(true); } },
            span { class: if emptying { "trash-delete-icon deleting" } else { "trash-delete-icon" }, TrashIcon { full } }
            if let Some(text) = count_text { span { class: "checkout-trash-count", "{text}" } }
        }
        if open() {
            div { class: "host-settings-backdrop", role: "presentation",
                ModalDialog { class: "checkout-trash-dialog", role: "dialog", aria_modal: "true", aria_labelledby: "checkout-trash-title", tabindex: "-1",
                    handle: DialogHandle::new(DIALOG),
                    focus: DialogFocus::remembered(".trash-close", Some(".trash-confirm-cancel, .trash-close"), false, Some(".trash-confirm-submit")),
                    on_escape: { let mut close = close.clone(); move |event: KeyboardEvent| { if !event.is_composing() { event.prevent_default(); event.stop_propagation(); close(); } } },
                    DialogHeader {
                        class: "checkout-trash-header",
                        title: rsx! { span { id: "checkout-trash-title", class: "checkout-trash-title",
                            span { class: "trash-delete-icon", TrashIcon { full } } "Trash"
                        } },
                        summary_class: "checkout-trash-total",
                        summary: rsx! { "{count} {checkouts(count)} · {space(total_bytes, total_partial)} on reachable hosts" },
                        close: rsx! {
                            // Keep this distinct from .trash-close: initial and
                            // recovery focus must still resolve footer Close.
                            button { class: "dialog-close trash-header-close", r#type: "button", aria_label: "Close trash",
                                onclick: { let mut close = close.clone(); move |_| close() }, "×" }
                        },
                    }
                    ScrollRegion { class: "checkout-trash-body",
                    handle: DialogHandle::new(DIALOG), selector: ".checkout-trash-body", name: "Archived checkouts",
                    recovery: RecoveryFocus::new(".trash-close", None, true, RecoveryDelay::OneFrame),
                    p { "Folders of managed checkouts whose last session was deleted. Before cleanup starts, you can copy work out of the folder shown under its host. Once cleanup is accepted, files may already be partly removed. Failed cleanup stays listed with its reason and Retry." }
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
                            let deleting = pending_hosts.contains_key(&host.id);
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
                                            }
                                            button { class: "btn btn-danger btn-outline trash-host-delete", disabled: busy || host_target.is_none(),
                                                onclick: move |_| {
                                                    // Reads the click-time listing only to
                                                    // open the band. The band then shows,
                                                    // and its answer deletes, what this froze.
                                                    if deletion.peek().is_empty()
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
                                                let retry = checkout.stuck.is_some();
                                                let action = if checkout.removing { "Removing…" } else if retry { "Retry" } else { "Delete" };
                                                let delete_label = if checkout.removing { format!("Removing {}", display_peer(&checkout.name)) } else if retry { format!("Retry cleanup of {}", display_peer(&checkout.name)) } else { format!("Delete {} permanently", display_peer(&checkout.name)) };
                                                rsx! {
                                                div { key: "{checkout.id}", class: "checkout-trash-entry",
                                                    div { class: "checkout-trash-entry-heading", strong { class: "peer-value font-mono", dir: "ltr", "{display_peer(&checkout.name)}" }
                                                        if !pending { if let Some(bytes) = checkout.bytes { span { "{space(bytes, false)}" } } }
                                                        button { class: "btn btn-danger btn-outline trash-entry-delete", aria_label: delete_label,
                                                            // An entry delete has no band of its own: keep it
                                                            // out of reach while a band asks about something
                                                            // else, and while moved rows settle (`SETTLE_MS`).
                                                            disabled: busy || gate.busy() || host_target.is_none() || shown.is_some() || !settled() || checkout.removing,
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
                                                            }, "{action}"
                                                        }
                                                    }
                                                    div { class: "checkout-trash-entry-meta", BranchIcon {} span { class: "peer-value", dir: "ltr", "{display_peer(&checkout.repository)}" } span { {archived_when(checkout.archived_at)} } }
                                                    if checkout.removing {
                                                        div { class: "checkout-trash-entry-meta", "Freeing space; some files may already be gone." }
                                                    } else if let Some(stuck) = &checkout.stuck {
                                                        div { class: "checkout-trash-issue checkout-trash-stuck",
                                                            strong { "Could not free space" }
                                                            " · " span { class: "font-mono", "{utc_timestamp(stuck.at)}" }
                                                            div { class: "peer-value", dir: "ltr", "{display_peer(&stuck.reason)}" }
                                                        }
                                                    }
                                                }
                                                }
                                                }
                                            }
                                            for (index, issue) in listing.issues.iter().enumerate() { Issue { key: "listed-{index}", issue: issue.clone() } }
                                        }
                                        for (index, notice) in host_notices.iter().enumerate() { Issue { key: "outcome-{index}", issue: notice.issue().clone() } }
                                        for path in paths { div { class: "checkout-trash-path", "On disk: " span { class: "peer-value", dir: "ltr", "{display_peer(&path)}" } } }
                                    }
                                }
                            }
                        }
                    }
                    }
                    if let Some(shown) = shown.as_ref().filter(|shown| shown.key().host.is_none()).cloned() {
                        // Global answers stay beside their pinned trigger. Long
                        // host names scroll locally, leaving the archive body
                        // reachable and retaining the band's existing focus owner.
                        ScrollRegion { class: "checkout-trash-global-confirmation",
                            handle: DialogHandle::new(DIALOG), selector: ".checkout-trash-global-confirmation", name: "Global checkout deletion confirmation",
                            recovery: RecoveryFocus::new(".trash-delete-all", Some(".trash-close"), true, RecoveryDelay::OneFrame),
                            ConfirmBand { shown, busy, submit_disabled: busy || gate.busy(), oncancel: band_cancel, onsubmit: band_submit }
                        }
                    }
                    DialogFooter { class: "checkout-trash-footer",
                        DialogActions {
                            button { class: "btn btn-neutral trash-close", onclick: { let mut close = close.clone(); move |_| close() }, "Close" }
                            button { class: "btn btn-danger trash-delete-all", disabled: busy || !all_ready || targets.is_empty(), onclick: move |_| { if deletion.peek().is_empty() { confirmation.open(Confirmation { host: None, targets: targets.clone() }, ()); } }, "Delete all" }
                        }
                    }
                }
            }
        }
    }
}

/// Capture the row before its confirmed reply removes it. Geometry belongs to
/// the persistent button's DOM, so unmounting the list cannot leak a global map.
/// A marked title band conceals the destination; it must not capture a flight
/// toward a zero-sized hidden button.
pub(super) fn capture_row(id: &str) {
    let id = serde_json::to_string(id).expect("string literal");
    document::eval(&format!(
        r#"(() => {{
        const id = {id}, button = document.querySelector('.checkout-trash-button');
        const row = Array.from(document.querySelectorAll('.session-row')).find(row => row.dataset.sessionId === id);
        if (!button || !row) return;
        button.__farhelmTrashOrigins?.delete(id);
        const target = button.getBoundingClientRect();
        if (!button.getClientRects().length || target.width <= 0 || target.height <= 0) return;
        (button.__farhelmTrashOrigins ??= new Map()).set(id, row.getBoundingClientRect());
    }})()"#
    ));
}

/// Only this client's affirmative archive reply plays the cue. A bare Delete,
/// a remote disappearance, a concealed destination and reduced motion all skip
/// flight and wiggle. Completion still consumes a prior capture when hidden.
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
        const visible = () => {{ const rect = button.getBoundingClientRect();
            return button.isConnected && button.getClientRects().length && rect.width > 0 && rect.height > 0; }};
        if (!{archived} || !visible() || matchMedia('(prefers-reduced-motion: reduce)').matches) return;
        const wiggle = () => {{ if (visible()) button.animate([
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

    /// Lost replies can cover several IDs. Resolving one or starting a later
    /// uncertain attempt for a neighbour must preserve every unresolved ID;
    /// only replacing or settling the final affected ID retires the notice.
    #[farhelm_testtrace::test]
    fn uncertain_notices_resolve_only_overlapping_ids() {
        let mut notice = TrashNotice::uncertain(&["a".into(), "b".into()], "lost reply".into());
        assert!(notice.resolve(&HashSet::from(["neighbour"])));
        assert!(notice.resolve(&HashSet::from(["a"])));
        assert!(notice.replace_uncertainty(&["neighbour".into()]));
        let TrashNotice::Uncertain { unresolved, .. } = &notice else {
            panic!("uncertain fixture");
        };
        assert_eq!(unresolved, &HashSet::from(["b".to_string()]));
        assert!(!notice.replace_uncertainty(&["b".into()]));

        let mut refusal = TrashNotice::Checkout(CheckoutTrashIssue {
            id: "refused".into(),
            path: String::new(),
            message: "retained refusal".into(),
        });
        assert!(refusal.replace_uncertainty(&["refused".into()]));
        assert!(refusal.resolve(&HashSet::from(["neighbour"])));
        assert!(!refusal.resolve(&HashSet::from(["refused"])));
    }

    /// Absence authorizes pruning only while a registry read is healthy.
    /// Loading and failed refreshes cannot turn missing evidence into removal.
    #[farhelm_testtrace::test]
    fn notice_pruning_requires_authoritative_host_absence() {
        let mut read = HostsRead::default();
        assert!(!host_removed(&read, 1));
        read.record(Ok(vec![host()]));
        assert!(!host_removed(&read, 1));
        assert!(host_removed(&read, 2));
        read.record(Err("registry unavailable".into()));
        assert!(!host_removed(&read, 1));
        assert!(!host_removed(&read, 2));
    }

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
            removing: false,
            stuck: None,
        };
        let mut state = HostTrash {
            incarnation: host.incarnation,
            loading: false,
            reply: Some(Ok(CheckoutTrashListing {
                removing: false,
                reports_retired: false,
                checkouts: vec![archive("shown")],
                issues: vec![],
            })),
            ..HostTrash::default()
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
            removing: false,
            stuck: None,
        };
        for (listing, expected_count, expected_size, expected_unchecked, expected_sidebar) in [
            (
                CheckoutTrashListing {
                    removing: false,
                    reports_retired: false,
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
                    removing: false,
                    reports_retired: false,
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
                    removing: false,
                    reports_retired: false,
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
                    removing: false,
                    reports_retired: false,
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
                ..HostTrash::default()
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
            removing: false,
            reports_retired: false,
            checkouts: (0..=MAX_SELECTION)
                .map(|id| ArchivedCheckout {
                    id: id.to_string(),
                    name: "old".into(),
                    repository: "fixture/repo".into(),
                    path: "/work/trash/old".into(),
                    archived_at: None,
                    bytes: Some(0),
                    removing: false,
                    stuck: None,
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
            ..HostTrash::default()
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
            removing: false,
            stuck: None,
        };
        let mut state = HostTrash {
            incarnation: host.incarnation,
            loading: false,
            reply: Some(Ok(CheckoutTrashListing {
                removing: false,
                reports_retired: false,
                checkouts: vec![checkout("selected"), checkout("neighbour")],
                issues: vec![CheckoutTrashIssue {
                    id: "issue".into(),
                    path: String::new(),
                    message: "unverified".into(),
                }],
            })),
            ..HostTrash::default()
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

    /// A cleanup receipt is not proof of reclaimed space on a capable peer.
    /// Retaining the running row keeps its count and completion poll alive,
    /// without allowing another immediate Retry; older peers keep their
    /// established optimistic disappearance instead. A diagnostic-only ID also
    /// needs that poll because its receipt can accept newly readable work.
    #[farhelm_testtrace::test]
    fn accepted_cleanup_keeps_capable_rows_until_an_authoritative_list() {
        let host = host();
        let checkout = farhelm_proto::ArchivedCheckout {
            id: "selected".into(),
            name: "old".into(),
            repository: "fixture/repo".into(),
            path: "/work/trash/.rm:old".into(),
            archived_at: None,
            bytes: Some(4096),
            removing: false,
            stuck: Some(farhelm_proto::CheckoutTrashStuck {
                reason: "permission denied".into(),
                at: 1_700_000_000,
            }),
        };
        for capable in [false, true] {
            let mut neighbour = checkout.clone();
            neighbour.id = "neighbour".into();
            neighbour.stuck = None;
            let mut state = HostTrash {
                incarnation: host.incarnation,
                reply: Some(Ok(CheckoutTrashListing {
                    reports_retired: capable,
                    checkouts: vec![checkout.clone(), neighbour],
                    ..CheckoutTrashListing::default()
                })),
                ..HostTrash::default()
            };
            accepted_cleanup(&mut state, &["selected".into()]);
            let listing = state.reply.as_ref().unwrap().as_ref().unwrap();
            assert_eq!(listing.checkouts.last().unwrap().id, "neighbour");
            if capable {
                assert_eq!(listing.checkouts.len(), 2);
                assert!(listing.removing && state.removing);
                assert!(listing.checkouts[0].removing);
                assert!(listing.checkouts[0].stuck.is_none());
                assert!(!listing.checkouts[1].removing);
                assert!(entry_target(&host, Some(&state), "selected").is_none());
                assert!(entry_target(&host, Some(&state), "neighbour").is_some());
            } else {
                assert_eq!(listing.checkouts.len(), 1);
                assert!(!listing.removing && !state.removing);
            }
        }
        let mut diagnostic_only = HostTrash {
            reply: Some(Ok(CheckoutTrashListing {
                reports_retired: true,
                issues: vec![farhelm_proto::CheckoutTrashIssue {
                    id: "selected".into(),
                    path: "/work/trash/old".into(),
                    message: "previously unavailable".into(),
                }],
                ..CheckoutTrashListing::default()
            })),
            ..HostTrash::default()
        };
        accepted_cleanup(&mut diagnostic_only, &["selected".into()]);
        let listing = diagnostic_only.reply.as_ref().unwrap().as_ref().unwrap();
        assert!(listing.checkouts.is_empty() && listing.issues.is_empty() && listing.removing);
        assert!(diagnostic_only.removing);
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
                removing: false,
                reports_retired: false,
                checkouts: vec![ArchivedCheckout {
                    id: "known".into(),
                    name: "old".into(),
                    repository: "fixture/repo".into(),
                    path: "/work/trash/old".into(),
                    archived_at: None,
                    bytes: None,
                    removing: false,
                    stuck: None,
                }],
                issues: vec![issue.clone(), issue],
            })),
            ..HostTrash::default()
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
                removing: false,
                stuck: None,
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

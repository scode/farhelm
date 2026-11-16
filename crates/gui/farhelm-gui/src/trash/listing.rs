//! What the Trash button and dialog know about each host's trash, and the plain rules over it: totals, wording, which
//! checkout may be deleted now, and what a delete's answer changes. No GPUI here, so every rule is tested without a
//! window.
//!
//! Ported from the old app's `list/trash.rs` at reference commit `24bc0580` (`HostTrash`, `listing_totals`, `space`,
//! `sidebar_summary`, `target`, `entry_target`, `accepted_cleanup`, the confirmation's and the tooltip's wording). One
//! of its parts is not here, on purpose, and another is here in a smaller form:
//!
//! - **Read generations.** The old page fenced every read with a generation that a delete advanced, so a count read
//!   before a delete could not put its rows back afterwards. Here commands are answered in the order they were sent
//!   (SPEC_impl.md, "The UI channel"), so a read sent before a delete is answered before it, and the button reads the
//!   host again once the delete settles. What remains is the order in which this app sees replies to different
//!   commands, which is not promised; [`HostTrash::serial`] keeps the newest read per host, so a reply to an older read
//!   is dropped. Read numbers come from one counter per UI session, never restarted for a host, so an answer from a
//!   host's earlier connection can never pass for one of its new connection's reads. DECISION (made while porting the
//!   trash, 2026-11-14, for the review gate to confirm): that per-host "newest read wins" replaces the generations. A
//!   pre-delete read that this app happens to see after the delete's own answer is older than the read the delete's
//!   answer sends, so it is dropped too.
//! - **Uncertain-outcome bookkeeping.** The old page kept a lost reply's whole frozen selection privately and resolved
//!   it ID by ID. Here an answer that was not a cleanup report is one [`Notice`] naming the IDs its delete sent (one
//!   for an entry's Delete, a host's whole frozen set for a confirmed delete), and a later report shrinks it by the IDs
//!   it answers, until none is left ([`record_outcome`]). That is the old rule without the private copy: the notice the
//!   dialog shows is the record.

#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;

use farhelm_proto::ui_channel::{HostStateView, HostView};
use farhelm_proto::{CheckoutTrashDeleted, CheckoutTrashIssue, CheckoutTrashListing};
use serde::Deserialize;

use crate::host_text::{HostSentence, HostText};
use crate::ticker;

/// The most IDs one cleanup request may name: the supervisor refuses larger batches. A host whose listing holds more is
/// shown but offers no cleanup at all, rather than one that silently leaves some out (the old app's `MAX_SELECTION`).
pub(crate) const MAX_SELECTION: usize = 10_000;

/// The body of `POST /api/checkout-trash/list`: the host's listing, and the helm's own failure to save shared notices
/// from it, which must stay visible without hiding the listing (the helm's `CheckoutTrashBody`).
#[derive(Debug, Deserialize)]
pub(crate) struct ListReply {
    #[serde(flatten)]
    pub listing: CheckoutTrashListing,
    #[serde(default)]
    pub notice_error: Option<String>,
}

/// One connected host's trash as this window last read it, for the connection (`incarnation`) the read was made on.
///
/// Read results are this window's own state, not pushed state: the helm keeps no trash cache and pushes only the
/// host's `trash_epoch`, which says when to read again.
#[derive(Default)]
pub(crate) struct HostTrash {
    /// The connection the listing belongs to. A listing never outlives its connection: a new incarnation starts over.
    pub incarnation: u64,
    /// The host's `trash_epoch` when the last read was sent: a pushed epoch that differs means the host's trash changed
    /// (a trash delete here, or a session Delete that archived a checkout, from this window, the command line or an
    /// agent) and is read again.
    pub epoch: u64,
    /// The last answer: the listing, or why there is none. `None` before the first answer. Shared, because every frame
    /// of the button and the dialog takes a [`Snapshot`] of it.
    pub listing: Option<Result<Rc<CheckoutTrashListing>, HostText>>,
    /// The helm could not save shared notices from the listing (the reply's `notice_error`).
    pub notice_error: Option<HostText>,
    /// The host's last report of active removal: the last successful listing's `removing`, or a delete this window
    /// had accepted since. A failed read keeps it, so a passing outage never looks like removal finished.
    pub removing: bool,
    /// A read is out. The listing shown meanwhile is the previous one, but it grants no cleanup until the read answers
    /// (the old app's "settled listings").
    pub loading: bool,
    /// The number of the newest read sent, from the button's one counter for the UI session, or 0 before the first. A
    /// reply to any other read is dropped (the module docs).
    pub serial: u64,
    /// The next recheck of a host that reported active removal; dropping it cancels it.
    pub recheck: Option<ticker::Deadline>,
}

/// A cleanup outcome the dialog keeps showing for a host until something settles it: a refusal the host gave for one
/// checkout, or a delete whose answer was not a cleanup report at all (the helm refused it, timed out on the host, or
/// sent something unreadable). SPEC.md: "Refusals the helm reports remain visible on the next dialog opening".
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Notice {
    /// The checkouts the outcome is about: the one a refusal names, or the ones an unconfirmed delete sent that no later
    /// report has answered yet. Never empty: a notice whose last ID is answered goes.
    pub ids: Vec<String>,
    /// The folder the host named, when it named one.
    pub path: Option<HostText>,
    /// The host's or the helm's words.
    pub message: HostText,
    /// The answer was not a cleanup report, so some files may already be gone; the dialog says so before the words.
    pub unconfirmed: bool,
}

/// Whether `host` can be read and cleaned now: connected, on a known connection.
pub(crate) fn connected(host: &HostView) -> bool {
    matches!(host.state, HostStateView::Connected { .. }) && host.incarnation != 0
}

/// "checkout" or "checkouts" for `count`.
pub(crate) fn checkouts(count: usize) -> &'static str {
    if count == 1 { "checkout" } else { "checkouts" }
}

/// How many distinct IDs a cleanup of the whole listing would name: checkouts and diagnostic-only records, each once.
pub(crate) fn selection_size(listing: &CheckoutTrashListing) -> usize {
    listing
        .checkouts
        .iter()
        .map(|entry| &entry.id)
        .chain(listing.issues.iter().map(|issue| &issue.id))
        .collect::<HashSet<_>>()
        .len()
}

/// A listing's counts and known size. Unchecked entries (diagnostics the host could not inspect) count apart from
/// checkouts, and any unknown size makes the byte sum partial, so an archive the host could not measure never reads as
/// zero bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Totals {
    pub count: usize,
    pub unchecked: usize,
    pub bytes: u64,
    pub partial: bool,
}

impl Totals {
    /// The host header's count: "2 checkouts", or "1 checkout · 2 unchecked entries".
    pub(crate) fn count_label(&self) -> String {
        let checked = format!("{} {}", self.count, checkouts(self.count));
        match self.unchecked {
            0 => checked,
            1 => format!("{checked} · 1 unchecked entry"),
            more => format!("{checked} · {more} unchecked entries"),
        }
    }
}

/// The totals of `listing` (see [`Totals`]).
pub(crate) fn totals(listing: &CheckoutTrashListing) -> Totals {
    Totals {
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

/// Allocated space for people: "46.0 MiB", or "4.0 KiB measured · partial" when part of it could not be measured, so
/// a partial sum never reads as the whole.
pub(crate) fn space(bytes: u64, partial: bool) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    let size = if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    };
    if partial {
        format!("{size} measured · partial")
    } else {
        size
    }
}

/// The folders to copy work out of, under one host: the parent of every listed checkout's and diagnostic's path, each
/// once, sorted. Computed on the raw paths; the dialog draws each as host text.
pub(crate) fn folders(listing: &CheckoutTrashListing) -> Vec<String> {
    listing
        .checkouts
        .iter()
        .map(|entry| entry.path.as_str())
        .chain(listing.issues.iter().map(|issue| issue.path.as_str()))
        .filter_map(|path| {
            std::path::Path::new(path)
                .parent()
                .map(|parent| parent.display().to_string())
        })
        .filter(|parent| !parent.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// What one delete request cleans on one host: the one checkout an entry's Delete or Retry names, or every ID a host
/// listed when its confirmation opened. Frozen then, with the connection it was listed on, so the helm refuses it if the
/// host reconnected since (SPEC_impl.md, "Archived-checkout trash": a single-entry delete "freezes only that checkout's
/// ID and its host incarnation at click time", and "Confirmations freeze deduplicated IDs and observed host
/// incarnations").
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    pub host: i64,
    pub incarnation: u64,
    pub ids: Vec<String>,
}

/// One host's share of a confirmed delete, frozen when its question opens: what is sent ([`Target`]), and what the
/// question says about it (the host's name and the listing's totals then). A listing that changes while the question
/// is up changes neither.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Batch {
    pub target: Target,
    pub name: HostText,
    pub totals: Totals,
}

/// The confirmation's lines for `batches` (the old `ConfirmBand`'s paragraphs): the checkouts, their size and the hosts
/// they are on, then the warning that work never committed or pushed is lost; and, when a host listed entries it could
/// not check, what cleanup does with those. The checkout lines are left out when every batch holds only unchecked
/// entries.
///
/// NOTE: Each old paragraph is two lines here, split where its host names end: a host sentence draws each of its parts
/// as a block of its own, so the text after a list of hosts started a new line anyway, with its "?" alone at its start.
/// Recorded in the parity file.
pub(crate) fn confirmation_text(batches: &[Batch]) -> Vec<HostSentence> {
    let count: usize = batches.iter().map(|batch| batch.totals.count).sum();
    let unchecked: usize = batches.iter().map(|batch| batch.totals.unchecked).sum();
    let bytes = batches
        .iter()
        .map(|batch| batch.totals.bytes)
        .fold(0, u64::saturating_add);
    let partial = unchecked > 0 || batches.iter().any(|batch| batch.totals.partial);
    // The hosts in their registry order, each once, joined as the old band joined them.
    let names = |which: fn(&Totals) -> usize, mut sentence: HostSentence| {
        let named = batches.iter().filter(|batch| which(&batch.totals) > 0);
        for (at, batch) in named.enumerate() {
            if at > 0 {
                sentence = sentence.text(", ");
            }
            sentence = sentence.host(&batch.name);
        }
        sentence
    };
    let mut lines = Vec::new();
    if count > 0 {
        lines.push(
            names(
                |totals| totals.count,
                HostSentence::new("trash-confirm-checkouts").text(format!(
                    "Delete {count} archived {} ({}) on ",
                    checkouts(count),
                    space(bytes, partial)
                )),
            )
            .text("?"),
        );
        lines.push(
            HostSentence::new("trash-confirm-warning")
                .text("Work in them that was never committed or pushed is lost permanently."),
        );
    }
    if unchecked > 0 {
        let noun = if unchecked == 1 { "entry" } else { "entries" };
        lines.push(
            names(
                |totals| totals.unchecked,
                HostSentence::new("trash-confirm-unchecked").text(format!(
                    "Attempt cleanup of {unchecked} unchecked {noun} on "
                )),
            )
            .text("."),
        );
        lines.push(HostSentence::new("trash-confirm-unchecked-rest").text(
            "Farhelm deletes any of these it can verify when cleanup runs; work in them that was never committed or \
             pushed is lost permanently. The rest stay in place and may be forgotten from the trash.",
        ));
    }
    lines
}

/// What a delete's answer was, as the button applies it: the host's cleanup report, or the words of an answer that
/// was not one.
pub(crate) type Outcome = Result<CheckoutTrashDeleted, HostText>;

/// Mark what a capable host accepted as removing, with any old failure cleared, so the row, the count and the size
/// stay until a later listing confirms the folder gone; an older host, which hides accepted rows itself, loses them at
/// once. Diagnostics for the accepted IDs go either way. A capable acceptance also starts the removal rechecks, even
/// for an ID with no row, since a failed refresh right after must not lose the outcome (the old `accepted_cleanup`).
pub(crate) fn accepted_cleanup(state: &mut HostTrash, ids: &[String]) {
    let Some(Ok(listing)) = state.listing.as_mut() else {
        return;
    };
    let listing = Rc::make_mut(listing);
    if listing.reports_retired {
        let accepted = !ids.is_empty();
        listing.removing |= accepted;
        state.removing |= accepted;
        for checkout in &mut listing.checkouts {
            if ids.contains(&checkout.id) {
                checkout.removing = true;
                checkout.stuck = None;
            }
        }
    } else {
        listing.checkouts.retain(|entry| !ids.contains(&entry.id));
    }
    listing.issues.retain(|issue| !ids.contains(&issue.id));
}

/// Apply the `outcome` of a delete that sent `sent` to the host's kept notices (the old `TrashNotice::resolve` and
/// `replace_uncertainty`). A cleanup report answers the sent IDs it names, removed or refused anew: each notice loses
/// those IDs, a notice left with none goes, and the report's refusals are added. Any other answer leaves every notice
/// but the earlier unconfirmed ones alone, takes the sent IDs out of those (going when none is left), and adds one
/// unconfirmed notice for all of them. An answer never settles a notice about checkouts it did not send.
pub(crate) fn record_outcome(notices: &mut Vec<Notice>, sent: &[String], outcome: &Outcome) {
    match outcome {
        Ok(deleted) => {
            let answered: HashSet<&str> = deleted
                .removed
                .iter()
                .map(String::as_str)
                .chain(deleted.issues.iter().map(|issue| issue.id.as_str()))
                .filter(|answered| sent.iter().any(|id| id == answered))
                .collect();
            for notice in notices.iter_mut() {
                notice.ids.retain(|id| !answered.contains(id.as_str()));
            }
            notices.extend(deleted.issues.iter().map(issue_notice));
        }
        Err(words) => {
            for notice in notices.iter_mut().filter(|notice| notice.unconfirmed) {
                notice.ids.retain(|id| !sent.contains(id));
            }
            notices.push(Notice {
                ids: sent.to_vec(),
                path: None,
                message: words.clone(),
                unconfirmed: true,
            });
        }
    }
    notices.retain(|notice| !notice.ids.is_empty());
}

/// A host's refusal for one checkout as a [`Notice`].
fn issue_notice(issue: &CheckoutTrashIssue) -> Notice {
    Notice {
        ids: vec![issue.id.clone()],
        path: (!issue.path.is_empty()).then(|| HostText::from_host(issue.path.as_str())),
        message: HostText::from_host(issue.message.as_str()),
        unconfirmed: false,
    }
}

/// Everything the button and the dialog draw, taken from the button's state and the store at one moment, so the two
/// agree and the rules over it are tested without a window.
#[derive(Clone, Debug, Default)]
pub(crate) struct Snapshot {
    /// The hosts collection is read and available. While it is not, nothing may be cleaned (the dialog says why), and
    /// counts are not claimed complete.
    pub hosts_known: bool,
    /// Why the hosts collection is unavailable, when the helm said.
    pub hosts_error: Option<HostText>,
    /// Every registered host, in the registry's order.
    pub hosts: Vec<HostSnapshot>,
    /// Whether a trash delete of this window's is in flight. One at a time: the trash control prevents overlapping
    /// deletes of its own (SPEC.md), so every entry's Delete waits for it.
    pub delete_in_flight: bool,
    /// The registered host that delete runs on, when the registry still lists it.
    pub deleting: Option<i64>,
}

impl Snapshot {
    /// The checkout `id` on `host` as the notices panel's Retry now would clean it now, or `None` while it may not: the
    /// registry is read, no trash delete of this window's is in flight (one at a time, as for every entry's Delete),
    /// and the host's listing shows the checkout stuck under its guards ([`HostSnapshot::retry_target`]).
    pub(crate) fn retry_target(&self, host: i64, id: &str) -> Option<Target> {
        if !self.hosts_known || self.delete_in_flight {
            return None;
        }
        self.hosts
            .iter()
            .find(|shown| shown.id == host)
            .and_then(|shown| shown.retry_target(id))
    }
}

/// One host in a [`Snapshot`].
#[derive(Clone, Debug)]
pub(crate) struct HostSnapshot {
    pub id: i64,
    /// The registry's name for it, which the old dialog showed ("this machine" for the local host).
    pub name: HostText,
    pub incarnation: u64,
    pub connected: bool,
    /// The last answer for this connection, if any, shared with the button's state.
    pub listing: Option<Result<Rc<CheckoutTrashListing>, HostText>>,
    /// No answer yet, or a read is out.
    pub pending: bool,
    pub removing: bool,
    pub notice_error: Option<HostText>,
    pub notices: Vec<Notice>,
    /// Other work holds the host's own key (its removal, say): the operation guard bulk cleanup shares.
    pub busy: bool,
}

impl HostSnapshot {
    /// The successful listing, if the last answer was one.
    pub(crate) fn listed(&self) -> Option<&CheckoutTrashListing> {
        self.listing.as_ref()?.as_ref().ok().map(Rc::as_ref)
    }

    /// The checkout `id` as an entry's Delete or Retry would clean it now, or `None` while this host's guards refuse:
    /// not connected, a read out, no successful listing, the listing over the batch limit, the checkout not listed or
    /// already removing, or the host's own key busy. Diagnostic-only records are never selected here; the bulk deletes
    /// clean them (the old `entry_target`).
    pub(crate) fn entry_target(&self, id: &str) -> Option<Target> {
        if !self.connected || self.pending || self.busy {
            return None;
        }
        let listing = self.listed()?;
        if selection_size(listing) > MAX_SELECTION {
            return None;
        }
        let checkout = listing.checkouts.iter().find(|entry| entry.id == id)?;
        (!checkout.removing).then(|| Target {
            host: self.id,
            incarnation: self.incarnation,
            ids: vec![checkout.id.clone()],
        })
    }

    /// The checkout `id` as the notices panel's Retry now would clean it now, or `None` while it may not (the old
    /// `retry_target`): the listing comes from a host that reports retained removal records, the checkout is listed with
    /// a failed cleanup (`stuck`), and the entry's own guards allow it ([`HostSnapshot::entry_target`]). SPEC.md: Retry
    /// now "is unavailable without a current connected host listing showing that same checkout stuck".
    pub(crate) fn retry_target(&self, id: &str) -> Option<Target> {
        let listing = self.listed()?;
        let stuck = listing.reports_retired
            && listing
                .checkouts
                .iter()
                .any(|entry| entry.id == id && entry.stuck.is_some());
        if stuck { self.entry_target(id) } else { None }
    }

    /// This host's share of a confirmed delete now, or `None` while its guards refuse (the old `target`): connected, its
    /// listing settled and within the batch limit, its own key free, and something listed. Every listed ID goes in,
    /// each once: the checkouts, then the diagnostic-only records, which cleanup rechecks and may delete once verifiable
    /// or forget without touching their folders.
    pub(crate) fn batch(&self) -> Option<Batch> {
        if !self.connected || self.pending || self.busy {
            return None;
        }
        let listing = self.listed()?;
        let mut seen = HashSet::new();
        let ids: Vec<String> = listing
            .checkouts
            .iter()
            .map(|entry| &entry.id)
            .chain(listing.issues.iter().map(|issue| &issue.id))
            .filter(|id| seen.insert(id.as_str()))
            .cloned()
            .collect();
        if ids.is_empty() || ids.len() > MAX_SELECTION {
            return None;
        }
        Some(Batch {
            target: Target {
                host: self.id,
                incarnation: self.incarnation,
                ids,
            },
            name: self.name.clone(),
            totals: totals(listing),
        })
    }

    /// Whether this host freed space or this window's delete runs on it: what turns the can over and names the host
    /// in the button's help. Only a connected host counts.
    fn freeing(&self, deleting: Option<i64>) -> bool {
        self.connected && (self.removing || deleting == Some(self.id))
    }
}

impl Snapshot {
    /// The connected hosts, which alone contribute counts and sizes.
    fn reachable(&self) -> impl Iterator<Item = &HostSnapshot> {
        self.hosts
            .iter()
            .filter(move |host| self.hosts_known && host.connected)
    }

    /// The archived checkouts known on reachable hosts, and whether any of them listed entries it could not check.
    pub(crate) fn count(&self) -> (usize, bool) {
        self.reachable().filter_map(HostSnapshot::listed).fold(
            (0, false),
            |(count, unchecked), listing| {
                (
                    count + listing.checkouts.len(),
                    unchecked || !listing.issues.is_empty(),
                )
            },
        )
    }

    /// Whether every reachable host answered with a settled listing within the batch limit: until then the count is a
    /// lower bound and says so.
    pub(crate) fn all_ready(&self) -> bool {
        self.hosts_known
            && self.reachable().all(|host| {
                !host.pending
                    && host
                        .listed()
                        .is_some_and(|listing| selection_size(listing) <= MAX_SELECTION)
            })
    }

    /// What the global Delete all would delete now, one batch per reachable host holding something, or `None` while it
    /// must wait (SPEC.md: "The global action skips unreachable hosts and waits until every reachable host can be
    /// read"): the registry unread, a reachable host's listing not settled or over the batch limit, a reachable host
    /// holding something whose own key is busy, or nothing anywhere. Unreachable hosts are skipped, never waited for.
    pub(crate) fn global_batches(&self) -> Option<Vec<Batch>> {
        if !self.all_ready() {
            return None;
        }
        let mut batches = Vec::new();
        for host in self.reachable() {
            match host.batch() {
                Some(batch) => batches.push(batch),
                None if host
                    .listed()
                    .is_some_and(|listing| selection_size(listing) > 0) =>
                {
                    return None;
                }
                None => {}
            }
        }
        (!batches.is_empty()).then_some(batches)
    }

    /// Whether the can is drawn open, and the words for what it holds (the old `sidebar_summary`): "empty", "3
    /// archived checkouts", or a count qualified as known only.
    pub(crate) fn summary(&self) -> (bool, String) {
        let (count, unchecked) = self.count();
        let full = count > 0 || unchecked;
        let words = if !self.all_ready() || unchecked {
            format!(
                "{count} known archived {}, some entries or hosts unchecked",
                checkouts(count)
            )
        } else if count == 0 {
            "empty".to_owned()
        } else {
            format!("{count} archived {}", checkouts(count))
        };
        (full, words)
    }

    /// The hosts freeing space, in registry order.
    pub(crate) fn freeing(&self) -> Vec<&HostSnapshot> {
        self.hosts
            .iter()
            .filter(|host| host.freeing(self.deleting))
            .collect()
    }

    /// Whether the can turns upside down: a delete of this window's is in flight, or a host reports freeing space.
    pub(crate) fn emptying(&self) -> bool {
        self.delete_in_flight || !self.freeing().is_empty()
    }

    /// Whether some cleanup outcome waits to be read in the dialog: a refusal or an unconfirmed answer, or the helm's
    /// failure to save shared notices.
    pub(crate) fn has_refusals(&self) -> bool {
        self.hosts
            .iter()
            .any(|host| !host.notices.is_empty() || host.notice_error.is_some())
    }

    /// The button's hover help (the old tooltip's wording): what the trash holds and what a click does, led by the
    /// hosts freeing space while the can is over, and followed by a pointer into the dialog while a refusal waits
    /// there. Host names are host values, so the help is a sentence.
    pub(crate) fn help(&self) -> HostSentence {
        let (_, summary) = self.summary();
        let (count, unchecked) = self.count();
        let mut help = HostSentence::new("checkout-trash-help");
        if self.emptying() {
            let freeing = self.freeing();
            if freeing.is_empty() {
                help = help.text("Emptying the trash. ");
            } else {
                help = help.text("Emptying the trash: freeing space on ");
                for (at, host) in freeing.iter().enumerate() {
                    if at > 0 {
                        help = help.text(", ");
                    }
                    help = help.host(&host.name);
                }
                help = help.text(". ");
            }
        }
        help = help.text(if !self.all_ready() || unchecked {
            format!("Trash: {summary}. Click to see what is known.")
        } else if count == 0 {
            "Trash: empty. When you delete the last session using a managed checkout, its folder moves here instead \
             of being deleted."
                .to_owned()
        } else {
            format!(
                "Trash: {summary}. A managed checkout moves here when the last session using it is deleted. Click to \
                 see them, or to delete them for good."
            )
        });
        if self.has_refusals() {
            help = help.text(" Some checkouts could not be deleted. Open the trash to see why.");
        }
        help
    }

    /// The known bytes on reachable hosts, and whether that sum is partial: a host still being read, failed, or with an
    /// unknown size, or the registry itself unknown.
    pub(crate) fn total(&self) -> (u64, bool) {
        let bytes = self
            .reachable()
            .filter_map(HostSnapshot::listed)
            .map(|listing| totals(listing).bytes)
            .fold(0, u64::saturating_add);
        let partial = !self.hosts_known
            || self.reachable().any(|host| {
                host.pending || host.listed().is_none_or(|listing| totals(listing).partial)
            });
        (bytes, partial)
    }
}

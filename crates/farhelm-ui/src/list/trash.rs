//! Show host-owned archives without treating a stale listing as deletion authority.
//!
//! Counts are event-driven. Opening the dialog asks for bounded disk usage;
//! confirmations freeze IDs and connection incarnations, while the supervisor
//! rechecks ownership before any mutation. Read generations prevent an earlier
//! count/measurement from restoring entries removed by this client's cleanup.

use crate::hosts::HostsRead;
use crate::icons::{BranchIcon, TrashIcon};
use crate::ops::{OpGuard, PaneGate};
use crate::peer::display_peer;
use crate::{ApiBase, Host, HostId, HostPhase};
use dioxus::prelude::*;
use farhelm_proto::{CheckoutTrashIssue, CheckoutTrashListing};
use std::collections::{HashMap, HashSet};

const DIALOG: &str = ".checkout-trash-dialog";
// The host rejects larger batches; disclose this rather than silently omitting
// a host from a global destructive confirmation.
const MAX_SELECTION: usize = 10_000;

/// Count wording stays grammatical at the destructive confirmation boundary.
fn checkouts(count: usize) -> &'static str {
    if count == 1 { "checkout" } else { "checkouts" }
}

/// A reply is meaningful only on the connection that supplied it. The retained
/// same-connection reply keeps the count steady during a fresh measurement.
#[derive(Clone, Default)]
struct HostTrash {
    incarnation: u64,
    reply: Option<Result<CheckoutTrashListing, String>>,
    loading: bool,
}

/// A confirmed set never grows with a subsequent listing or a new archive.
/// Diagnostic IDs are included too, so unusable ownership can be forgotten.
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

/// One in-place confirmation, bound to the host band or the global footer.
#[derive(Clone, PartialEq)]
struct Confirmation {
    host: Option<HostId>,
    targets: Vec<Target>,
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

/// Collapse repeated diagnostic IDs into one selection, keeping every message
/// visible separately. Sizes with any unknown checkout remain explicitly partial.
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
    Some(Target {
        host: host.id,
        incarnation: host.incarnation,
        name: host.name.clone(),
        ids,
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
        partial: listing.checkouts.iter().any(|entry| entry.bytes.is_none()),
    })
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
fn archived_when(seconds: Option<i64>) -> String {
    let Some(seconds) = seconds else {
        return "archive time unknown".into();
    };
    let (year, month, day) = farhelm_proto::time::civil_from_days(seconds.div_euclid(86400));
    let time = seconds.rem_euclid(86400);
    format!(
        "archived {year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        time / 3600,
        time % 3600 / 60
    )
}

/// Release isolation before handing focus back to the persistent trash button.
fn return_focus() {
    document::eval(&format!(
        "{} requestAnimationFrame(() => document.querySelector('.checkout-trash-button')?.focus({{preventScroll:true}}));",
        crate::modal_isolation::release_js(DIALOG)
    ));
}

/// A host message stays escaped and direction-isolated, including a path that
/// was preserved or partially cleaned. Removed IDs do not suppress diagnostics.
#[component]
fn Issue(issue: CheckoutTrashIssue) -> Element {
    rsx! { div { class: "checkout-trash-issue", role: "status",
        if !issue.path.is_empty() { div { class: "peer-value", dir: "ltr", "{display_peer(&issue.path)}" } }
        div { class: "peer-value", dir: "ltr", "{display_peer(&issue.message)}" }
    } }
}

/// Confirm the frozen visible set in place, including unverified entries.
/// Those may be forgotten without deleting their folders, a distinction the
/// user needs before choosing permanent cleanup.
#[component]
fn ConfirmBand(
    confirm: Confirmation,
    busy: bool,
    submit_disabled: bool,
    oncancel: EventHandler<()>,
    onsubmit: EventHandler<()>,
) -> Element {
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
    rsx! { div { class: "checkout-trash-confirm", role: "group", aria_label: "confirm permanent checkout deletion",
        onmounted: move |_| { document::eval("requestAnimationFrame(() => document.querySelector('.checkout-trash-confirm .trash-confirm-cancel')?.focus({preventScroll:true}))"); },
        if count > 0 { p { "Delete {count} archived {checkouts(count)} ({space(bytes, partial)}) on " span { class: "peer-value", dir: "ltr", "{names}" } "? Work in them that was never committed or pushed is lost permanently." } }
        if unchecked > 0 { p { "Attempt cleanup of {unchecked} unchecked {unchecked_noun} on {unchecked_names}. Folders Farhelm cannot verify stay in place; unusable entries may be forgotten from the trash." } }
        div { class: "checkout-trash-actions",
            button { class: "btn btn-neutral trash-confirm-cancel", disabled: busy, onclick: move |_| oncancel.call(()), "cancel" }
            button { class: "btn btn-danger trash-confirm-submit", disabled: submit_disabled, onclick: move |_| onsubmit.call(()), "delete permanently" }
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
    let mut open = use_signal(|| false);
    let mut state = use_signal(HashMap::<HostId, HostTrash>::new);
    let mut generation = use_signal(|| 0_u64);
    let mut refresh = use_signal(|| 0_u64);
    let mut confirmation = use_signal(|| None::<Confirmation>);
    let mut notices = use_signal(HashMap::<HostId, Vec<CheckoutTrashIssue>>::new);
    let mut remaining = use_signal(|| 0_usize);
    // The claim outlives all host tasks and drops with this component on cancel.
    let mut claim = use_signal(|| None::<OpGuard>);
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
        if remaining() != 0 {
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
    let count: usize = current_hosts
        .iter()
        .filter(|host| hosts_known && connected(host))
        .filter_map(|host| {
            state
                .read()
                .get(&host.id)
                .filter(|entry| entry.incarnation == host.incarnation)
                .and_then(|entry| entry.reply.as_ref())
                .and_then(|reply| reply.as_ref().ok())
                .map(|reply| reply.checkouts.len())
        })
        .sum();
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
    let busy = remaining() != 0;
    let mut gate = gate;
    let delete_base = base;
    let submit = use_callback(move |_: ()| {
        if *remaining.peek() != 0 {
            return;
        }
        let Some(confirm) = confirmation.peek().clone() else {
            return;
        };
        // Host changes invalidate a displayed confirmation before dispatch;
        // the helm repeats the incarnation fence for changes it sees first.
        let valid = {
            let hosts = hosts.peek();
            hosts.refresh_error().is_none()
                && confirm.targets.iter().all(|target| {
                    hosts.hosts().unwrap_or_default().iter().any(|host| {
                        host.id == target.host
                            && connected(host)
                            && host.incarnation == target.incarnation
                    })
                })
        };
        if !valid {
            confirmation.set(None);
            refresh += 1;
            return;
        }
        let Some(guard) = gate.claim_guard() else {
            return;
        };
        claim.set(Some(guard));
        remaining.set(confirm.targets.len());
        generation += 1; // supersede every pre-delete count and size reply
        confirmation.set(None);
        notices.write().clear();
        for target in confirm.targets {
            let base = delete_base.clone();
            spawn(async move {
                let result = crate::api::empty_checkout_trash(
                    &base,
                    target.host,
                    target.incarnation,
                    &target.ids,
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
                        notices.write().insert(target.host, reply.issues);
                    }
                    Err(error) => {
                        notices.write().insert(target.host, vec![CheckoutTrashIssue {
                            id: String::new(), path: String::new(),
                            message: format!("The cleanup reply could not be confirmed; some folders may already be deleted. Rechecking the trash: {error}"),
                        }]);
                    }
                }
                remaining -= 1;
                if *remaining.peek() == 0 {
                    claim.set(None);
                    refresh += 1;
                    document::eval(
                        "requestAnimationFrame(() => document.querySelector('.trash-close')?.focus({preventScroll:true}))",
                    );
                }
            });
        }
    });
    let mut close = move |_| {
        if *remaining.peek() != 0 {
            return;
        }
        if confirmation.peek().is_some() {
            confirmation.set(None);
            return;
        }
        return_focus();
        open.set(false);
    };
    let tooltip = if !all_ready {
        format!(
            "trash: {count} known archived {} on reachable hosts; some hosts have not been checked. Click to see what is known.",
            checkouts(count)
        )
    } else if count == 0 {
        "trash: empty. When you delete the last session using a managed checkout, its folder moves here instead of being deleted.".into()
    } else {
        format!(
            "trash: {count} archived {}. A managed checkout moves here when the last session using it is deleted. Click to see them, or to delete them for good.",
            checkouts(count)
        )
    };
    let accessible_name = if !all_ready {
        format!(
            "checkout trash, {count} known archived {}, some hosts unchecked",
            checkouts(count)
        )
    } else if count == 0 {
        "checkout trash, empty".into()
    } else {
        format!("checkout trash, {count} archived {}", checkouts(count))
    };
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
                    .is_none_or(|entry| entry.loading || !matches!(&entry.reply, Some(Ok(_))))
            })
        || targets
            .iter()
            .any(|target| target.partial || target.unchecked > 0);
    rsx! {
        button {
            class: if count == 0 { "btn btn-neutral checkout-trash-button" } else { "btn btn-neutral checkout-trash-button full" },
            r#type: "button", aria_label: accessible_name, aria_haspopup: "dialog", "data-count": "{count}",
            "data-tooltip": tooltip, disabled: gate.busy(),
            onclick: move |_| { if !gate.busy_now() { notices.write().clear(); confirmation.set(None); open.set(true); } },
            TrashIcon { full: count > 0 }
            if count > 0 { span { class: "checkout-trash-count", "{count}" } }
        }
        if open() {
            div { class: "host-settings-backdrop", role: "presentation",
                div { class: "checkout-trash-dialog", role: "dialog", aria_modal: "true", aria_labelledby: "checkout-trash-title", tabindex: "-1",
                    onmounted: move |_| crate::hosts::settings_dialog::install_dialog_with_selector(DIALOG, ".trash-close", Some(".trash-confirm-cancel, .trash-close")),
                    onkeydown: move |event| { if event.key() == Key::Escape && !event.is_composing() { event.prevent_default(); event.stop_propagation(); close(()); } },
                    div { class: "checkout-trash-header",
                        h2 { id: "checkout-trash-title", TrashIcon { full: count > 0 } "trash" }
                        button { class: "btn btn-neutral trash-close", disabled: busy, onclick: move |_| close(()), "close" }
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
                            let show = !connected(host) || pending || listing.is_none_or(|listing| !listing.checkouts.is_empty() || !listing.issues.is_empty()) || !host_notices.is_empty();
                            let name = display_peer(&host.name);
                            let host_count = listing.map_or_else(|| "count unknown".into(), |listing| format!("{} {}", listing.checkouts.len(), checkouts(listing.checkouts.len())));
                            let host_size = if connected(host) && hosts_known && pending { "measuring…".into() } else { listing.map_or_else(|| "size unknown".into(), |listing| space(listing.checkouts.iter().filter_map(|entry| entry.bytes).fold(0, u64::saturating_add), listing.checkouts.iter().any(|entry| entry.bytes.is_none()))) };
                            let paths = listing.map(|listing| listing.checkouts.iter().map(|entry| &entry.path)
                                .chain(listing.issues.iter().map(|issue| &issue.path))
                                .filter_map(|path| std::path::Path::new(path).parent().map(|path| path.display().to_string()))
                                .collect::<std::collections::BTreeSet<_>>()).unwrap_or_default();
                            rsx! {
                                if show {
                                    section { key: "{host.id}", class: if connected(host) { "checkout-trash-host" } else { "checkout-trash-host unreachable" }, "data-host-id": "{host.id}",
                                        div { class: "checkout-trash-host-header",
                                            div { strong { class: "peer-value", dir: "ltr", "{name}" } div { "{host_count} · {host_size}" } }
                                            button { class: "btn btn-danger btn-outline trash-host-delete", disabled: busy || host_target.is_none(),
                                                onclick: move |_| {
                                                    if *remaining.peek() == 0
                                                        && hosts.peek().refresh_error().is_none()
                                                        && let Some(target) = target(&action_host, state.peek().get(&action_host.id))
                                                    {
                                                        confirmation.set(Some(Confirmation { host: Some(action_host.id), targets: vec![target] }));
                                                    }
                                                },
                                                "delete all on " span { class: "peer-value", dir: "ltr", "{name}" }
                                            }
                                        }
                                        if let Some(confirm) = confirmation().filter(|confirm| confirm.host == Some(host.id)) {
                                            ConfirmBand { confirm, busy, submit_disabled: busy || gate.busy(), oncancel: move |_| { confirmation.set(None); }, onsubmit: submit }
                                        }
                                        if !connected(host) { p { "not reachable, so they can't be checked or deleted" } }
                                        if let Some(Err(error)) = entry.and_then(|entry| entry.reply.as_ref()) { div { class: "checkout-trash-issue peer-value", dir: "ltr", "{display_peer(error)}" } }
                                        if let Some(listing) = listing {
                                            if selection_size(listing) > MAX_SELECTION { p { "This host exceeds the 10,000-entry cleanup limit. Cleanup is unavailable for this listing." } }
                                            for checkout in listing.checkouts.iter() {
                                                div { key: "{checkout.id}", class: "checkout-trash-entry",
                                                    div { class: "checkout-trash-entry-heading", strong { class: "peer-value", dir: "ltr", "{display_peer(&checkout.name)}" }
                                                        if !pending { if let Some(bytes) = checkout.bytes { span { "{space(bytes, false)}" } } }
                                                    }
                                                    div { class: "checkout-trash-entry-meta", BranchIcon {} span { class: "peer-value", dir: "ltr", "{display_peer(&checkout.repository)}" } span { "{archived_when(checkout.archived_at)}" } }
                                                }
                                            }
                                            for (index, issue) in listing.issues.iter().enumerate() { Issue { key: "listed-{index}", issue: issue.clone() } }
                                        }
                                        for (index, issue) in host_notices.iter().enumerate() { Issue { key: "outcome-{index}", issue: issue.clone() } }
                                        for path in paths { div { class: "checkout-trash-path", "on disk: " span { class: "peer-value", dir: "ltr", "{display_peer(&path)}" } } }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(confirm) = confirmation().filter(|confirm| confirm.host.is_none()) {
                        ConfirmBand { confirm, busy, submit_disabled: busy || gate.busy(), oncancel: move |_| { confirmation.set(None); }, onsubmit: submit }
                    }
                    div { class: "checkout-trash-footer",
                        span { "{count} {checkouts(count)} · {space(total_bytes, total_partial)} on reachable hosts" }
                        button { class: "btn btn-danger trash-delete-all", disabled: busy || !all_ready || targets.is_empty(), onclick: move |_| { if *remaining.peek() == 0 { confirmation.set(Some(Confirmation { host: None, targets: targets.clone() })); } }, "delete all" }
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

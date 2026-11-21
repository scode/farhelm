//! Updating a remote host: the host menu's Update and the rerun of a failed one, a row's inline `↑ Update` button, and
//! the Hosts heading's Update all, which all send the same request.
//!
//! SPEC.md, "Topology" (the Update paragraphs and Update all) and "Session list" (the update button, Too new). Ported
//! from the old app's automatic-update path in `provisioning.rs` (`prepare`'s Update arm, `planning_refusal`,
//! `available_remote_updates`) and the update parts of `hosts.rs` (`host_update_kind`, `host_update_title`,
//! `remote_update_available`, the inline `host-update-button`), at reference commit `24bc0580`. Here's the TLDR of its
//! rules:
//!
//! - **No confirmation.** SPEC.md: "the user's Update click ... is the authorization: no plan is shown for
//!   confirmation." [`start`] sends the plan request (`POST /api/hosts/{id}/update` without a body, which the helm
//!   answers 200 with a one-use plan within its host-wait bound), then from its completion the confirming request
//!   naming that plan, which the helm accepts (202) once the run has started. Both are the operation store's work under
//!   the host's key ([`Work::UpdateHost`]), so they go on whatever view is open, and the row reads "Updating…" while
//!   they are out ([`super::progress`]). The run's progress and outcome are the helm's pushed host progress, which the
//!   row shows; nothing here waits for them.
//! - **The same request everywhere.** The menu's Update and its Re-run of a failed Update, the inline button and Update
//!   all each call [`start`] for a host whose Update is available ([`available`], the host menu's own rule): a remote
//!   host with nothing running or in flight on it that does not run a newer Farhelm. Update all ([`start_all`]) sends it
//!   for every listed host that rule admits at the click, so a busy host is skipped rather than queued and the helm's own
//!   machine, whose Update is greyed, is left out; each host keeps its own progress and result.
//! - **Refused before sending.** While a session launch is in flight anywhere ([`super::launch_in_flight`]: the
//!   launcher's New or Clone, a session's Restart or Replace), an Update is not sent, and the Hosts panel's line says
//!   why once per press, naming the host ("Update <host>: not started, …", or "Re-run <host>: …" for the menu's Re-run
//!   of a failed Update), or "Update all: not started, …" for the heading's press, as Remove, setup and Re-run say
//!   theirs ([`super::setup::not_started`]). This is the migration's rule, decided by its orchestrator on 2026-11-14
//!   after the old app refused a host change during a launch and the reverse: each side refuses its own action while
//!   the other's work is in flight in the operation store, with a reason, and no shared lock. A press that meets other
//!   work under the host's key sends nothing and records nothing: no control offers Update on a busy host, so only a
//!   press racing a frame gets there.
//! - **A refused plan** (a newer host build, a newer database schema, a supervisor below the upgrade floor with its
//!   manual remedy, or any other reason the helm gives) opens the host refusal dialog ([`super::refusal`]) through the
//!   mark taken at the press, which the layer manager refuses once the user has moved on since. A refusal the dialog
//!   showed is dropped from the operation store's record under the host's key ([`operations::Operations::forget`]), so the row
//!   stays as it was, as in the old app; one that opened no dialog stays there, under the row, which opens for it. A
//!   refused confirming request shows under the row only, as the old app kept its one-click Update's submission
//!   outcomes on the row. Either refusal, and a plan reply that names no plan, also warns the host's rail avatar while
//!   Hosts is closed ([`super::status::failed`]), as the old app raised its attention on every failed planning or
//!   submission, whether or not the dialog showed it.
//! - **The inline button** ([`button`]): on a row whose host (a remote host of any kind whose Update is available, as
//!   the old app drew it and SPEC.md says) runs an older build ("Old version", amber, optional) or an older protocol
//!   ("Needs update", red, required) and whose Update is available, the outlined `↑ Update` beside the phase words. Its
//!   hover names both builds (and both protocols for a skew), says whether updating is optional or required, and that
//!   clicking installs the helm's version ([`hover`]). Pressing it starts the Update and moves focus to the row's menu
//!   toggle, since the row's progress takes the button's place.
//!
//! The refusal dialog draws Remove for a refusal the helm marks "remove instead" ([`super::refusal`]); the helm sets
//! that marker only on an uninstall plan's refusals, and the old Update never offered Remove, so an Update's refusal
//! draws Close alone.

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::ui_channel::{HostStateView, HostView};
use gpui::{
    AnyElement, AnyWindowHandle, App, FocusHandle, IntoElement, ParentElement as _, SharedString,
    Styled as _, Window, div,
};
use serde::Deserialize;
use serde_json::json;

use super::identity::display_name;
use super::listed;
use super::offers::{self, Update};
use super::refusal;
use super::setup_sign_in;
use crate::components::{Button, HoverHelp, Role, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText};
use crate::operations::{self, Command, OpKey, Outcome, Work};
use crate::session;

#[cfg(test)]
pub(crate) mod tests;

/// How a refusal without words names the Update's requests, with the host's id hidden.
const SHOWN: &str = "POST /api/hosts/[host]/update";

// ---- The requests ----------------------------------------------------------------------------------------------------

/// `POST /api/hosts/{id}/update` without a body for host `host`: the helm probes the host and answers with a one-use
/// plan for updating it, or refuses.
fn plan_command(host: i64) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::UpdateHost,
        Method::Post,
        path(host)?,
        None,
        SHOWN,
    ))
}

/// The same route naming the plan `probe_id`: the helm consumes that plan, once, and starts the run.
fn confirm_command(host: i64, probe_id: &str) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::UpdateHost,
        Method::Post,
        path(host)?,
        Some(json!({ "probe_id": probe_id })),
        SHOWN,
    ))
}

/// The Update route of host `host`.
fn path(host: i64) -> Result<ApiPath, PathError> {
    ApiPath::api(&["hosts", &HostView::key(host), "update"])
}

/// The part of the helm's plan reply the confirmation needs: the plan's one-use id. The rest (the plan and its steps)
/// is for a confirmation Update never shows.
#[derive(Deserialize)]
struct Plan {
    probe_id: String,
}

// ---- Starting an Update ----------------------------------------------------------------------------------------------

/// Whether `host`'s Update is available, given its retained run `progress` and whether work on it is in flight: what
/// the host menu offers as a live Update ([`offers::offers`]). The inline button and Update all ask the same. The
/// installed desktop app's check on the helm's own machine is not this Update ([`offers::menu_offers`]), so neither
/// draws it nor sends it.
pub(crate) fn available(
    host: &HostView,
    progress: Option<&farhelm_proto::provisioning::ProvisioningView>,
    in_flight: bool,
) -> bool {
    offers::offers(host, progress, in_flight).update == Some(Update::Available)
}

/// Update host `host`, pressed with `mark` taken in the press's handler: the plan request, then the confirming request
/// (the module docs). While a session launch is in flight it sends nothing and the Hosts panel's line says "Update
/// <host>: not started, …"; a press that meets other work on the host's key sends and records nothing; nothing happens
/// when the window is inert.
pub(crate) fn start(host: i64, mark: Mark, window: &mut Window, cx: &mut App) {
    start_as("Update ", host, mark, window, cx);
}

/// The host menu's Re-run of `host`'s failed Update: [`start`], whose line during a launch names the press the user
/// made, "Re-run <host>: not started, …", as a failed setup's Re-run does.
pub(crate) fn rerun(host: i64, mark: Mark, window: &mut Window, cx: &mut App) {
    start_as("Re-run ", host, mark, window, cx);
}

/// [`start`] for a press named `verb` (with its trailing space) on the panel's line when a launch holds it back.
fn start_as(verb: &'static str, host: i64, mark: Mark, window: &mut Window, cx: &mut App) {
    if super::launch_in_flight(cx) {
        let Some(name) = listed(host, cx).map(|view| display_name(&view)) else {
            return;
        };
        let line = not_started_line().text(verb).host(&name);
        super::setup::not_started(line.text(format!(": {}", super::LAUNCH_IN_FLIGHT)), cx);
        return;
    }
    start_with(host, mark, window.window_handle(), cx);
}

/// Update every host of `hosts` whose Update is available now ([`available`]), each as [`start`] would, with `mark`
/// taken in the press's handler. The others are skipped: busy, the helm's own machine, too new, or no longer listed.
pub(crate) fn start_all(hosts: &[i64], mark: Mark, window: &mut Window, cx: &mut App) {
    // One line for the press, not one per host it would have updated.
    if super::launch_in_flight(cx) {
        let line = not_started_line().text(format!("Update all: {}", super::LAUNCH_IN_FLIGHT));
        super::setup::not_started(line, cx);
        return;
    }
    let handle = window.window_handle();
    // Decided before any is sent: starting one host's Update changes nothing another's availability depends on.
    let available: Vec<i64> = hosts
        .iter()
        .copied()
        .filter(|host| is_available(*host, cx))
        .collect();
    for host in available {
        start_with(host, mark, handle, cx);
    }
}

/// Whether `host`'s Update is available as the store and the operation store hold it now; `false` when the window is
/// inert or the registry does not list it.
fn is_available(host: i64, cx: &App) -> bool {
    let Some(store) = session::current_store(cx) else {
        return false;
    };
    let key = HostView::key(host);
    let collections = store.read(cx).collections();
    let in_flight = session::current_operations(cx)
        .is_some_and(|operations| operations.read(cx).in_flight(&OpKey::host(&key)).is_some());
    collections
        .get::<HostView>(&key)
        .is_some_and(|view| available(view, collections.get(&key), in_flight))
}

/// The Hosts panel's line for an Update not started during a launch, empty, under its own id.
fn not_started_line() -> HostSentence {
    HostSentence::new("hosts-update-not-started")
}

/// [`start`] for `window`, once no launch holds it back.
fn start_with(host: i64, mark: Mark, window: AnyWindowHandle, cx: &mut App) {
    let key = OpKey::host(&HostView::key(host));
    let command = match plan_command(host) {
        Ok(command) => command,
        Err(error) => {
            tracing::debug!(?error, "host update plan not built");
            return;
        }
    };
    let planned = key.clone();
    let started = setup_sign_in::plan(key, command, mark, window, cx, move |result, mark, cx| {
        let settled = match result {
            Ok(settled) => settled,
            Err(reason) => {
                note(&planned, reason, cx);
                super::status::failed(host, super::status::Action::Update, cx);
                return;
            }
        };
        match &settled.outcome {
            Outcome::Done(body) => confirm(host, &planned, body.clone(), mark, window, cx),
            // The dialog opens only through the press's mark, which the layer manager refuses once the user has
            // moved on. Shown there, the refusal is dropped from the store's record under the host's key, so the
            // row neither shows it again nor opens for it, as the old app kept it in the dialog alone; not shown,
            // it stays under the row, the one place left to show it.
            Outcome::Refused(refusal) => {
                super::status::failed(host, super::status::Action::Update, cx);
                refusal::show_and_forget(host, "Update host", refusal, &planned, &mark, window, cx);
            }
            // The pending reply has already been handled by the sign-in continuation.
            Outcome::Accepted(_) => {}
        }
    });
    match started {
        // A line about an Update not started belongs to that press; one has started since.
        Ok(()) => super::setup::forget_not_started(cx),
        // Busy: no control offers Update while the host's key is held, so this is a press racing a frame, and it records
        // nothing. Retired: a reset's generic connection-lost notice is the report.
        Err(not_started) => tracing::debug!(?not_started, "host update not sent"),
    }
}

/// The plan request for `host` answered with `body`: send the confirming request naming its plan under `key`. The
/// store records a refusal of it under the key, where the row shows it, and an accepted one needs nothing: the run is
/// already pushed. A reply that names no plan says so under the row.
fn confirm(
    host: i64,
    key: &OpKey,
    body: Option<serde_json::Value>,
    mark: Mark,
    window: AnyWindowHandle,
    cx: &mut App,
) {
    let plan = match operations::decode::<Plan>(Ok(body), SHOWN) {
        Ok(plan) => plan,
        Err(problem) => {
            note(key, problem, cx);
            super::status::failed(host, super::status::Action::Update, cx);
            return;
        }
    };
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let command = match confirm_command(host, &plan.probe_id) {
        Ok(command) => command,
        Err(error) => {
            tracing::debug!(?error, "host update confirmation not built");
            return;
        }
    };
    let started = operations.update(cx, |operations, cx| {
        operations.start(key, command, cx, move |settled, cx| {
            if let Outcome::Refused(_) = settled.outcome {
                super::status::failed(host, super::status::Action::Update, cx);
            } else {
                setup_sign_in::accepted(&settled.outcome, false, mark, window, cx);
            }
        })
    });
    if let Err(not_started) = started {
        tracing::debug!(?not_started, "host update confirmation not sent");
    }
}

/// Record `message` under `key` where the row shows a refusal of work on the host: from the plan request's completion,
/// when its reply names no plan, so no later work on the key is running yet.
fn note(key: &OpKey, message: HostText, cx: &mut App) {
    if let Some(operations) = session::current_operations(cx) {
        operations.update(cx, |operations, cx| {
            operations.note(key, Work::UpdateHost, message, cx)
        });
    }
}

// ---- The inline button -----------------------------------------------------------------------------------------------

/// How urgent a host's update is, by what its phase says about its build: optional for a connected host on an older
/// build, required for one refused at the handshake with an older protocol. Ported from the old `host_update_kind`.
/// The inline button's tier and the rail's update badge ([`super::status::update_badge`]) both follow it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Urgency {
    Optional,
    Required,
}

/// The urgency of `state`'s update, or `None` when its phase establishes none: a current, newer or unknown build, an
/// equal protocol, or a host not connected. A newer build wins over the old-build hint, as it does for the phase words.
pub(crate) fn urgency(state: &HostStateView) -> Option<Urgency> {
    match state {
        HostStateView::Connected {
            old_version: true,
            newer_version: false,
            ..
        } => Some(Urgency::Optional),
        HostStateView::VersionSkew {
            peer_protocol,
            our_protocol,
            ..
        } if peer_protocol < our_protocol => Some(Urgency::Required),
        _ => None,
    }
}

/// The inline button's hover for `state`, and the attention list's Update's, as the old app gave both the same
/// `host_update_title`; `None` where the button draws none ([`urgency`]). The host's build and the helm's
/// (with both protocols for a skew), whether updating is optional or required, and that clicking installs the helm's
/// version, in the old `host_update_title`'s words. Host builds are host text. A connected host's helm build is this
/// build's, as for the Too new hover (`identity::phase::hover`).
pub(crate) fn hover(state: &HostStateView) -> Option<HostSentence> {
    const CLICK: &str = "; click to update this host to the helm's version";
    let sentence = HostSentence::new("host-update-help").text("This host runs Farhelm ");
    match (state, urgency(state)?) {
        (HostStateView::Connected { build_version, .. }, Urgency::Optional) => Some(
            sentence
                .host(&HostText::from_host(build_version.as_str()))
                .text(format!(
                    "; this helm runs {}; update optional{CLICK}",
                    farhelm_proto::BUILD_VERSION
                )),
        ),
        (
            HostStateView::VersionSkew {
                peer_protocol,
                peer_build,
                our_protocol,
                our_build,
                ..
            },
            Urgency::Required,
        ) => Some(
            sentence
                .host(&HostText::from_host(peer_build.as_str()))
                .text(format!(" (protocol {peer_protocol}); this helm runs "))
                .host(&HostText::from_host(our_build.as_str()))
                .text(format!(
                    " (protocol {our_protocol}); update required{CLICK}"
                )),
        ),
        _ => None,
    }
}

/// The shared Update name for a single host. Keyboard and screen-reader users can tell which host a button
/// affects in either list, with the same escaping as the host name shown on its row.
pub(crate) fn accessible_name(host: &HostView) -> SharedString {
    HostSentence::new("host-update-name")
        .text("Update ")
        .host(&display_name(host))
        .accessible_name()
}

/// Host `host`'s inline `↑ Update` (`.host-update-button`), or `None` where the row draws none: its Update is not
/// available (`available`, from the panel's frame) or its phase establishes no urgency. Named "Update <name>", with
/// [`hover`] as its hover help. Pressing it starts the Update and moves focus to `toggle`, the row's menu toggle, as
/// the old button did: the row's progress takes the button's place, which would leave focus nowhere.
pub(crate) fn button(
    host: &HostView,
    available: bool,
    toggle: Option<FocusHandle>,
) -> Option<AnyElement> {
    let urgency = urgency(&host.state).filter(|_| available)?;
    let help = hover(&host.state)?;
    let id = host.id;
    let button = Button::new(
        SharedString::from(format!("host-update-{id}")),
        "Update",
        HoverHelp::sentence(help).ring_radius(crate::theme::metrics::RADIUS_SM - gpui::px(1.)),
        Role::Button,
    )
    .tier(Tier::HostUpdate {
        required: urgency == Urgency::Required,
    })
    .accessible_name(accessible_name(host))
    .content(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(gpui::px(3.))
            .child("↑")
            .child("Update"),
    )
    .on_click(move |_, window, cx| {
        let mark = Mark::take(cx);
        start(id, mark, window, cx);
        if let Some(toggle) = &toggle {
            focus::focus_control(toggle, &mark, window, cx);
        }
    });
    Some(button.into_any_element())
}

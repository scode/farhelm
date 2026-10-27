//! The UI half of provisioning: inspect, authorize, submit, then follow the
//! host-scoped run through the fleet feed.
//!
//! The helm is the authority for every meaningful decision here. It decides
//! whether a probe found a supervisor, positively established absence, or
//! reached a manual-only target; it renders the exact setup confirmation from
//! the plan the executor will consume; and it excludes overlapping runs across
//! every browser. This module keeps those answers intact rather than
//! reconstructing them from paths or action tags the UI happens to know.
//!
//! ## Setup confirms; a remote update authorizes by the click
//!
//! Initial setup (ADD, including its reruns and the automatic local offer)
//! still shows the helm-rendered confirmation and submits only on an explicit
//! confirm. A remote UPDATE submits automatically instead: the Update click
//! is the authorization, and the plan it mints flows straight into the same
//! binding-checked, single-consumption submission the confirm button enters
//! ([`submit_plan`]). Planning failure submits nothing; repeated clicks
//! coalesce into the one accepted update; a retarget, a removal, or a foreign
//! running update invalidates the unsubmitted intent instead of queueing
//! behind it. The POST itself is never retried automatically: once the plan
//! leaves this client, its token stays spent whatever the reply says.
//!
//! ## Why `OpLock` ends at acceptance
//!
//! A provision or update can take a minute. Holding the page token for that
//! minute would disable unrelated creates and host changes without excluding
//! another browser, while the helm already owns the real host-scoped lock.
//! Submission therefore claims [`OpLock`] only around the POST and releases
//! it as soon as the submission request completes: accepted identity,
//! accepted-but-unreadable body, or refusal. Progress is a read after that
//! point, driven by the existing fleet feed and its bounded fallback.
//!
//! ## Local setup is the same operation over another transport
//!
//! The reserved local row probes with `{kind: "local"}`. There is no
//! SSH-to-self fallback. A provisionable reply replaces the old manual-start
//! hint as the primary remedy while keeping that command beneath it; a
//! manual-only reply (including no usable systemd user manager) leaves the
//! command as the whole remedy.
//!
//! ## Per-host disclosure rides beside the global checkbox
//!
//! Effective details for a row is the global checkbox OR that row's automatic
//! disclosure, and the two are owned separately: the checkbox is the user's
//! preference and nothing here writes it for an update, while the panel
//! publishes its own row's disclosure and compact update status beside one
//! another (see [`update_auto_disclosed`]). A running update reports progress
//! in the host row rather than opening its step list; failures and unresolved
//! outcomes still open that row. While that inline status shows, the folded
//! row's one-line run trace is not drawn, since it would only repeat it.
//! Success clears the automatic disclosure only for the exact tracked run
//! id — never on a 202, a planning success, a bare Connected, or an
//! unrelated run's completion.
//! A change-of-host warning has one narrower exception: once the host is
//! connected at this helm's reported build, its advice to check the version
//! is satisfied. Clearing that warning proves nothing about the detached
//! update and does not settle any other uncertainty.

use std::collections::{HashMap, HashSet};
use web_time::Instant;

use dioxus::prelude::*;

use crate::anchored_popovers::{MovementTicket, use_rendered_movement};
use crate::api::{
    PlanSteps, PreferenceValue, ProbeResponse, ProvisioningAccepted, ProvisioningOperation,
    ProvisioningStatus, ProvisioningSubmission, ProvisioningView, SubmissionError,
    fetch_provisioning, plan_host_uninstall, plan_host_update, probe_local_host, probe_ssh_host,
    provision_host, store_preference, uninstall_host, update_host,
};
use crate::feed::{fallback_polls_now, fallback_sleep, use_feed_reader};
use crate::hosts::settings_dialog;
use crate::hosts::{HostRefusal, HostRefusalAction, HostRefusalEntry};
use crate::list::SharedPreferences;
use crate::modal_dialog::{
    DialogFocus, DialogHandle, FocusTarget, ModalDialog, RecoveryDelay, RecoveryFocus, ScrollRegion,
};
use crate::ops::{ConfirmSlot, ConfirmToken, OpGuard, OpLock, ReadGate, use_confirm_slot};
use crate::peer::{
    DetailPart, PeerBlock, PeerBlockBody, PeerLine, PeerText, display_identity, display_peer,
};
use crate::reader::{SurfaceReader, Trigger, request_read};
use crate::{ApiBase, Host, HostId, HostKind};

/// The provisioning commands one host row currently contributes to its
/// actions menu.
///
/// This is presentation state, not an authority for whether a run may start.
/// The permanently mounted provisioning component still rechecks its own
/// lifecycle state and the page operation lock when it consumes a request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ProvisioningMenuState {
    pub(crate) rerun: Option<ProvisioningOperation>,
    pub(crate) automatic_setup: bool,
    pub(crate) update: bool,
    /// Uninstall is offered: a remote row with no run, plan or update
    /// intent in flight. A failed uninstall offers this again rather than a
    /// rerun, since choosing uninstall again is how it continues.
    pub(crate) uninstall: bool,
    /// A plan is in flight: ADD planning or a displayed offer, a live
    /// automatic-update intent in any phase, or a retained accepted/observed
    /// run that has not reached a terminal state. While set, the menu offers
    /// no new provisioning command and disables the in-flight one.
    pub(crate) planning: bool,
    /// ADD or Uninstall is preparing or holding a confirmation. Unlike a
    /// one-click Update, it can still open a modal and must exclude Settings
    /// on every host until the question is settled.
    pub(crate) confirmation_may_open: bool,
}

/// What decides a host row's provisioning offers: [`ProvisioningMenuState::offered`]'s
/// input, gathered by the row's provisioning component from its run, plan
/// and host state.
#[derive(Debug, Clone, Copy, Default)]
struct MenuFacts {
    /// A run is in progress on the host.
    run_active: bool,
    /// The operation of the host's last run, when it failed.
    failed_operation: Option<ProvisioningOperation>,
    /// The host kind accepts provisioning from the panel at all.
    update_allowed: bool,
    /// The host kind can be uninstalled from the panel (remote rows only).
    uninstall_allowed: bool,
    /// The local row's supervisor is not running (setup, not Update).
    local_setup: bool,
    /// ADD or Uninstall preparation has not returned yet, including the
    /// local row's automatic setup probe.
    confirmation_planning: bool,
    /// A prepared plan still awaits confirmation or automatic submission.
    has_pending_plan: bool,
    /// An Update intent or a tracked run is still owned by this panel.
    update_owned: bool,
    /// The local setup's automatic retry is allowed.
    can_retry_local_setup: bool,
    /// The host runs a newer farhelm than the helm
    /// (`hosts::runs_newer_version`).
    too_new: bool,
}

impl ProvisioningMenuState {
    /// The offers a row's menu makes, as one pure function of its facts so
    /// the rules can be tested without mounting the component.
    /// It also publishes the narrower confirmation lifecycle separately:
    /// Settings on other hosts need not wait for a one-click Update.
    ///
    /// Update is never offered for a host running a newer farhelm than the
    /// helm: it would install the helm's older build over it (SPEC.md:
    /// Update never downgrades a host), and the helm refuses it anyway.
    fn offered(facts: MenuFacts) -> Self {
        let MenuFacts {
            run_active,
            failed_operation,
            update_allowed,
            uninstall_allowed,
            local_setup,
            confirmation_planning,
            has_pending_plan,
            update_owned,
            can_retry_local_setup,
            too_new,
        } = facts;
        // A pending plan is conservatively modal-producing, even for the
        // defensive Update-plan branch. Ordinary Updates never enter it.
        let confirmation_may_open = confirmation_planning || has_pending_plan;
        let plan_in_flight = confirmation_may_open || update_owned;
        ProvisioningMenuState {
            // A failed Update is not offered again on a too-new host either:
            // the rerun is the same downgrade. A failed uninstall is never a
            // rerun: its own menu item continues it.
            rerun: (update_allowed && !plan_in_flight)
                .then_some(failed_operation)
                .flatten()
                .filter(|operation| !(too_new && *operation == ProvisioningOperation::Update))
                .filter(|operation| *operation != ProvisioningOperation::Uninstall),
            automatic_setup: !run_active
                && update_allowed
                && local_setup
                && !plan_in_flight
                && failed_operation.is_none()
                && can_retry_local_setup,
            update: !run_active && update_allowed && !local_setup && !plan_in_flight && !too_new,
            uninstall: !run_active && uninstall_allowed && !plan_in_flight,
            planning: plan_in_flight,
            confirmation_may_open,
        }
    }
}

/// The rendered facts that make one collapsed provisioning trace distinct.
///
/// Progress-step churn is deliberately absent: fixed surfaces need dismissal
/// only when the one-line trace itself appears, changes, or disappears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProvisioningTraceShape {
    pub(crate) operation: ProvisioningOperation,
    pub(crate) status: ProvisioningStatus,
}

/// What the hosts panel says once an uninstall this window confirmed has
/// removed its host from the list: the host's name and where its data
/// stayed. Both are peer text and render through the escaping path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UninstallNotice {
    pub(crate) host_name: String,
    pub(crate) state_dir: String,
    /// The run the row was showing when this notice was registered: on a
    /// retry, the earlier failed run. Its failure says nothing about the
    /// attempt this notice belongs to, so a read of it arriving late must
    /// not withdraw the notice; only a different run failing does.
    pub(crate) superseded_run: Option<String>,
}

/// One row-menu provisioning command, with the row it was aimed at.
///
/// The binding is captured at click time, in the row that rendered the menu
/// item. A click queued behind a retarget must not become an update of the
/// retargeted row, so consumption compares this against the live binding and
/// drops the request on mismatch instead of planning against new facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActionRequest {
    pub(crate) operation: ProvisioningOperation,
    pub(crate) binding: HostBinding,
}

/// What consuming one queued menu request does.
///
/// A request is never left to start whenever a flag happens to clear: it is
/// either consumed now (by starting work, by joining the live intent, or by
/// being dropped as stale) or explicitly refused so it stays queued for a
/// later pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestDecision {
    /// Begin planning for this request.
    Plan,
    /// A live update intent already covers this click; drop the duplicate
    /// rather than queueing a second update behind the first.
    Coalesce,
    /// The click's row no longer matches; drop without starting anything.
    Stale,
    /// A blocker that may clear (page lock, ADD planning, a live update
    /// intent) stands; leave the request queued for another pass.
    Refuse,
}

/// Decide what one queued menu request becomes, synchronously.
///
/// `update_owned` is whether an automatic update currently owns this row: a
/// live intent in any phase, or a retained accepted/observed run that has not
/// reached a terminal state (see [`update_owned`]). An update never refuses
/// on the page lock — planning mutates nothing, and the submission claim
/// retries reactively — but it does refuse behind ADD planning (whose
/// completion it would otherwise race) and coalesces into live ownership
/// rather than minting a second plan. An ADD supersedes nothing while an
/// update owns the row. A merely displayed ADD offer is superseded by either
/// operation, as before.
fn decide_request(
    request: &ActionRequest,
    current: &HostBinding,
    update_owned: bool,
    add_planning: bool,
    page_busy: bool,
) -> RequestDecision {
    if request.binding != *current {
        return RequestDecision::Stale;
    }
    match request.operation {
        ProvisioningOperation::Update if update_owned => RequestDecision::Coalesce,
        ProvisioningOperation::Update if add_planning => RequestDecision::Refuse,
        ProvisioningOperation::Update => RequestDecision::Plan,
        // UNINSTALL shows a plan and waits for confirmation, as ADD does,
        // so it shares ADD's blockers and its supersede rule.
        ProvisioningOperation::Add | ProvisioningOperation::Uninstall
            if update_owned || add_planning || page_busy =>
        {
            RequestDecision::Refuse
        }
        // A second ADD supersedes a merely displayed offer, as before: the
        // older plan is dropped unsent and planning starts over.
        ProvisioningOperation::Add | ProvisioningOperation::Uninstall => RequestDecision::Plan,
    }
}

/// Whether this row-menu operation submits without confirmation.
///
/// Remote UPDATE is always automatic. ADD is handled by the add-host dialog,
/// which may submit after probing when the shared helm preference records the
/// user's permanent answer; that preference must not leak into this menu
/// helper or make a local row's unsupported UPDATE look automatic.
fn automatic_update(operation: ProvisioningOperation, binding: &HostBinding) -> bool {
    operation == ProvisioningOperation::Update && binding.kind.updates_automatically()
}

/// Returns the row-menu request only after the provisioning lifecycle consumes it.
///
/// A refusal leaves the request queued. The surrounding effect subscribes to
/// every blocker [`decide_request`] names, so releasing one gives the same
/// command another pass instead of losing the click. Returning the consumed
/// value lets the caller defer its signal write until removal is real,
/// avoiding a false reactive notification on refusal.
fn accepted_request(
    requests: &HashMap<HostId, ActionRequest>,
    host_id: HostId,
    mut begin: impl FnMut(&ActionRequest) -> bool,
) -> Option<ActionRequest> {
    let request = requests.get(&host_id).cloned()?;
    begin(&request).then_some(request)
}

/// Registry facts that make a displayed plan belong to this exact row.
///
/// The helm revalidates the same boundary before mutation. Keeping the UI
/// binding as well prevents a stale confirmation from remaining actionable
/// under a retargeted or newly adopted row while that refusal round trip is
/// still avoidable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostBinding {
    id: HostId,
    kind: HostKind,
    destination: Option<String>,
    identity: Option<String>,
    remote_farhelm: Option<String>,
    remote_state_dir: Option<String>,
    incarnation: u64,
}

impl From<&Host> for HostBinding {
    fn from(host: &Host) -> Self {
        Self {
            id: host.id,
            kind: host.kind,
            // Bindings compare dial coordinates and identity, never their display spellings.
            destination: host
                .destination
                .as_ref()
                .map(|value| value.raw().to_owned()),
            identity: host.identity.as_ref().map(|value| value.raw().to_owned()),
            remote_farhelm: host
                .remote_farhelm
                .as_ref()
                .map(|value| value.raw().to_owned()),
            remote_state_dir: host
                .remote_state_dir
                .as_ref()
                .map(|value| value.raw().to_owned()),
            incarnation: host.incarnation,
        }
    }
}

/// Whether a submitted update's row still names the target it ran against.
///
/// Full [`HostBinding`] equality is the right check before submission, but
/// an update normally changes its own connection incarnation mid-run — the
/// backend's attach step deliberately waits for a fresh one — so requiring
/// it after acceptance would drop a healthy run just before completion.
/// What identifies the target is the registry id, the kind, and the dial
/// coordinates with the recorded identity; incarnation churn and learned
/// install paths do not break continuity. An update can also learn the
/// first identity: a protocol-skewed host was registered without an
/// established identity, and the update's reconnect supplies it. This is a
/// directional allowance, not permission to replace or lose a known
/// identity. Callers retaining a run must pin that first learned identity
/// so a later replacement cannot reuse the allowance. Anything else is a
/// real target change, and old run evidence must not be reinterpreted as
/// the new target's history.
fn same_update_target(before: &HostBinding, now: &HostBinding) -> bool {
    before.id == now.id
        && before.kind == now.kind
        && before.destination == now.destination
        && (before.identity.is_none() || before.identity == now.identity)
}

/// Explain a broken update continuity check using the host's visible facts.
///
/// The caller already decided that continuity broke. Entry/kind changes
/// have no useful pair of values; otherwise destination takes precedence
/// when both fields changed. Destinations stay raw: the diagnostic's peer
/// renderer escapes the whole sentence once, including invisible controls.
/// Identities are spelled with [`display_identity`] first, because that
/// renderer is not injective (an actual U+202E and the literal text
/// `<U+202E>` come out alike) and the user compares the two identities to
/// tell whether a different install answered. Its output is plain ASCII,
/// which the renderer passes through unchanged and which carries no text
/// direction of its own, so the identities need no isolated runs either.
fn changed_host_warning(before: &HostBinding, now: &HostBinding, submitting: bool) -> String {
    let when = if submitting {
        "while its update was being sent"
    } else {
        "while it was being updated"
    };
    let changed = if before.id != now.id || before.kind != now.kind {
        format!("this host's entry changed {when}")
    } else if before.destination != now.destination {
        format!(
            "this host's ssh destination changed from ‹{}› to ‹{}› {when}",
            before
                .destination
                .as_deref()
                .unwrap_or("no ssh destination reported"),
            now.destination
                .as_deref()
                .unwrap_or("no ssh destination reported"),
        )
    } else {
        let meaning = if now.identity.is_some() {
            "a different Farhelm install now answers at its ssh destination"
        } else {
            "the install at its ssh destination can no longer be identified"
        };
        let spell = |identity: Option<&str>| {
            identity.map_or_else(|| "no identity reported".to_string(), display_identity)
        };
        format!(
            "this host's identity changed from ‹{}› to ‹{}› {when} ({meaning})",
            spell(before.identity.as_deref()),
            spell(now.identity.as_deref()),
        )
    };
    let consequence = if submitting {
        "Farhelm can't tell whether the update started"
    } else {
        "Farhelm stopped following that update and can't tell how it ended or which install it reached"
    };
    format!(
        "{changed}, so {consequence}; check the farhelm version this host shows now, and update it again if it isn't current"
    )
}

/// Where one accepted automatic-update click stands before its POST.
///
/// Planning success stores the one-use plan and moves to [`IntentPhase::WaitingForClaim`];
/// the claim effect then moves to [`IntentPhase::Submitting`] at the moment
/// it takes the page token. Any POST outcome ends the intent: the plan stays
/// spent and the tracked run or the retained diagnostic carries onward. A
/// retarget can also end it while the POST is still outstanding, which is why
/// progress suppression follows [`OutstandingSubmission`] — the POST's own
/// lifetime — rather than this phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntentPhase {
    Planning,
    WaitingForClaim,
    Submitting,
}

/// One accepted automatic-update click: its epoch, its row, its phase.
///
/// The epoch is what makes invalidation total. Bumping the row's counter
/// (on a newer click, a retarget, or a foreign running run) orphans every
/// in-flight planning future at once: a stale completion finds an epoch or
/// a phase it no longer owns and restores nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct UpdateIntent {
    epoch: u64,
    binding: HostBinding,
    phase: IntentPhase,
    /// The minted plan, present only while waiting for the submission claim.
    /// Removed at claim, before the POST leaves; never reused, never replayed.
    plan: Option<PendingPlan>,
}

/// One update POST in flight, tracked apart from the invalidatable intent.
///
/// The binding watcher drops the planning intent on a retarget while the
/// POST remains outstanding — and a newer click can mint a fresh intent
/// behind it — so suppression keyed on the intent would end while the
/// submission is still unanswered, and a progress read could adopt the old
/// target's run as the new target's work. This outlives the intent: set at
/// claim, cleared only when this POST's outcome installs, so progress
/// commits stay suppressed, with retry demand retained, until the outcome
/// is in. Row removal drops it with the component; it is never retried or
/// replayed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OutstandingSubmission {
    epoch: u64,
    /// Keep the first learned identity even after the intent is invalidated:
    /// acceptance must reject a second identity learned while the POST waits.
    binding: HostBinding,
}

/// How the tracked run reached this client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunSource {
    /// This client's submission was accepted; `epoch` is the submitting
    /// intent, which is what lets its correlated success supersede that
    /// intent's diagnostics.
    Submitted { epoch: u64 },
    /// Read off the progress route: another client's run, or a run whose
    /// acceptance reply this client never decoded. Followed as observed
    /// work, and its success settles nothing but its own disclosure.
    Observed,
}

/// One host/run pair this panel is following, whatever the connection does.
///
/// Retained across ordinary incarnation churn (see [`same_update_target`]);
/// only a real target change, an authoritative success, or row removal ends
/// it. Run ids are opaque: inequality detects a different run, never its
/// age, so a newer observation replaces the tracked pair outright.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TrackedRun {
    host: HostId,
    run_id: String,
    operation: Option<ProvisioningOperation>,
    source: RunSource,
    /// The binding that submitted or adopted the run; the continuity anchor.
    /// A first learned identity is pinned here by the binding watcher, so
    /// later identity changes cannot inherit the unknown-identity allowance.
    binding: HostBinding,
    /// Aggregate status of the newest applicable read naming this run, so a
    /// superseding observation can tell a finished run from one whose
    /// outcome was never seen.
    last_status: Option<ProvisioningStatus>,
    /// Client-local monotonic origin for elapsed display; the helm's view has
    /// no timestamps, and a reload intentionally starts a fresh observation.
    started_at: Instant,
}

/// The compact status the host row can show without opening the step list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HostUpdateProgress {
    /// Planning or acceptance is underway, before a usable run snapshot exists.
    Pending(UpdatePendingPhase),
    /// A setup or uninstall plan request is in flight before its dialog opens.
    PendingConfirmed(ProvisioningOperation),
    /// A tracked run has a live progress snapshot and a local elapsed origin.
    Running(UpdateProgressSummary),
}

/// Which lifecycle stage a pending `updating…` status stands for.
///
/// The user sees the same `updating…` in every stage; the stage is published
/// only as the row's `data-update-phase` attribute. It exists because the
/// row no longer opens its details while an update runs, so the planning,
/// waiting-for-claim, and submitting indicators inside those details are
/// hidden, and browser tests that hold one of those boundaries still need to
/// observe which one the UI actually reached. A response arriving does not
/// prove that: the UI may not have processed it yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdatePendingPhase {
    /// The update click was accepted and its plan request is in flight.
    Planning,
    /// A minted plan is stored, waiting for the page operation lock.
    WaitingForClaim,
    /// The plan's submission POST is outstanding.
    Submitting,
    /// A live tracked UPDATE run exists, but no running snapshot for that
    /// exact run has been read yet: the gap between a 202 and the first
    /// read, or a displayed view that names some other run.
    AwaitingProgress,
}

impl UpdatePendingPhase {
    /// Stable attribute value, a browser handle like `data-provisioning-status`.
    pub(crate) fn attribute(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::WaitingForClaim => "waiting",
            Self::Submitting => "submitting",
            Self::AwaitingProgress => "awaiting-progress",
        }
    }
}

impl UpdateProgressSummary {
    /// The word the row's inline status and its hover popup use for this
    /// run: "setting up", "updating" or "uninstalling".
    pub(crate) fn verb(&self) -> &'static str {
        match self.operation {
            ProvisioningOperation::Add => "setting up",
            ProvisioningOperation::Uninstall => "uninstalling",
            ProvisioningOperation::Update => "updating",
        }
    }
}

/// The small per-host summary published from the provisioning panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpdateProgressSummary {
    /// The exact tracked run id, so replacing a run also replaces its clock.
    pub(crate) run_id: String,
    /// The run's operation chooses the inline verb; the step list itself
    /// keeps the helm's words for every kind of run.
    pub(crate) operation: ProvisioningOperation,
    /// First executor-ordered `running` step, absent when the view names none.
    pub(crate) current_step: Option<String>,
    /// Every executor step in the snapshot, in executor order, for the hover
    /// popup's full list. `current` marks the entry `current_step` names.
    pub(crate) steps: Vec<UpdateStepLine>,
    /// Steps in a terminal state: completed, skipped, degraded, or failed.
    pub(crate) done: usize,
    /// All executor steps in the snapshot, including pending work.
    pub(crate) total: usize,
    /// Client-local monotonic start; never a helm timestamp or persisted value.
    pub(crate) started_at: Instant,
}

/// One run step in the hover popup, with the helm's warning or failure reason.
///
/// The id is a stable handle and the name is for people. A folded row can
/// hide the expanded rail for this run, so the popup must retain a reason
/// that changes what the host will do, such as lingering falling back to login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpdateStepLine {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) message: Option<String>,
    pub(crate) current: bool,
}

/// Borrow the readable name's original bytes, or the stable id for old snapshots.
///
/// The folded summary still owns plain strings and the expanded rail escapes
/// this borrow at render time. Empty names retain the existing id fallback;
/// whitespace-only names are data and must not be trimmed into that fallback.
fn progress_step_name(step: &crate::api::ProvisioningStep) -> &str {
    step.name
        .as_ref()
        .map(crate::peer::PeerText::raw)
        .filter(|name| !name.is_empty())
        .unwrap_or(&step.step)
}

/// Use a result word that distinguishes no-op and degraded work from failure.
///
/// These words come from the helm's finite StepStatus enum, not command output.
/// Preserve unknown future words verbatim rather than refusing a whole snapshot.
pub(crate) fn progress_step_status(status: &str) -> &str {
    match status {
        "skipped" => "unchanged",
        "degraded" => "warning",
        other => other,
    }
}

/// One unresolved update diagnostic, kept apart from observed progress.
///
/// An unrelated completed run, an idle view, or a successful GET never
/// clears these; only a correlated newer success, an explicit superseding
/// attempt, or row removal does. The one exception is a change-of-host
/// warning: a connected host at this helm's build satisfies its version
/// advice, without establishing what happened to the detached update.
#[derive(Debug, Clone, PartialEq, Eq)]
struct UpdateDiagnostic {
    /// The intent epoch (or the counter value, for post-intent events) that
    /// produced this. A correlated success clears diagnostics at or below
    /// its own submitting epoch; anything newer survives it.
    epoch: u64,
    /// Whether a fresh explicit attempt leaves this standing. Submission
    /// uncertainty — a lost submission reply, a malformed or lost
    /// acceptance, a run that vanished into idle, a superseded observation
    /// — outlives the next click and is superseded only by that attempt's
    /// correlated success. Planning failures, definite refusals, and
    /// foreign-run explanations describe the attempt they came from and a
    /// new attempt replaces them.
    sticky: bool,
    /// Only continuity warnings may clear from connected/version evidence.
    /// Other sticky uncertainties still need their correlated update result.
    host_changed: bool,
    text: String,
}

/// Offer clearable host advice only when no earlier submission uncertainty stands.
///
/// An observed run can coexist with an earlier update's unresolved outcome.
/// Retargeting still detaches that run, but replacing the earlier diagnostic
/// with version-clearable advice would let Connected erase uncertainty that
/// only a correlated update result can settle. Keep its text and epoch intact.
fn changed_host_diagnostic(
    held: Option<&UpdateDiagnostic>,
    epoch: u64,
    before: &HostBinding,
    now: &HostBinding,
    submitting: bool,
) -> Option<UpdateDiagnostic> {
    if held.is_some_and(|held| held.sticky && !held.host_changed) {
        return None;
    }
    Some(UpdateDiagnostic {
        epoch,
        sticky: true,
        host_changed: true,
        text: changed_host_warning(before, now, submitting),
    })
}

/// Read the helm build from the evidence already used by the build-skew UI.
///
/// A reported mismatch names the helm rather than this bundle. Otherwise
/// a seen stamp proves the bundle's build matched. Silent and unseen helms
/// establish nothing. This inherits the skew latch's first-disagreement
/// semantics and display-length cap; it is not a new version tracker.
fn known_helm_build(skew: Option<&crate::skew::Skew>, seen: bool) -> Option<&str> {
    match skew {
        Some(crate::skew::Skew::Reported(build)) => Some(build),
        None if seen => Some(crate::skew::CLIENT_BUILD),
        Some(crate::skew::Skew::Silent) | None => None,
    }
}

/// Whether version advice is satisfied, without settling any update result.
///
/// Exact raw build equality is the contract: neither age flag being set
/// can also mean the versions were not comparable. A bare connection,
/// unknown helm build, or another diagnostic is insufficient.
fn change_warning_resolved(
    warning: &UpdateDiagnostic,
    state: &crate::HostPhase,
    helm_build: Option<&str>,
) -> bool {
    warning.host_changed
        && matches!(state, crate::HostPhase::Connected { build_version, .. }
            if Some(build_version.raw()) == helm_build)
}

/// Whether a progress view is authoritative success for the tracked run.
///
/// All of: decoded and applicable (the caller gates that), naming the exact
/// tracked nonempty run id, reporting UPDATE, aggregate Completed. Notably
/// NOT a 202, a planning success, host Connected, every visible step looking
/// finished, or the idle view's Completed-with-no-run-id. Older-run success
/// can never satisfy this for a newer tracked pair, because the run id
/// differs — which is why run ids are compared, never sorted.
fn authoritative_update_success(view: &ProvisioningView, tracked: &TrackedRun) -> bool {
    !tracked.run_id.is_empty()
        && tracked.operation == Some(ProvisioningOperation::Update)
        && view.operation == Some(ProvisioningOperation::Update)
        && view.status == ProvisioningStatus::Completed
        && view.run_id.as_deref() == Some(tracked.run_id.as_str())
}

/// Whether this row's automatic disclosure is currently owed.
///
/// Only Update failure or uncertainty opens its row automatically. Running
/// Update progress stays inline; setup and uninstall use the separate
/// per-run decision below.
fn update_auto_disclosed(
    tracked: Option<&TrackedRun>,
    error: Option<&UpdateDiagnostic>,
    warning: Option<&UpdateDiagnostic>,
    progress_error: Option<&str>,
) -> bool {
    tracked.is_some_and(|run| {
        run.operation == Some(ProvisioningOperation::Update)
            && (run.last_status == Some(ProvisioningStatus::Failed) || progress_error.is_some())
    }) || error.is_some()
        || warning.is_some()
}

/// One setup or uninstall run's client-local expansion choice.
///
/// The run id is the boundary: folding one run cannot hide the next one.
/// A failed result reopens for its diagnostic even if the user folded while
/// it was running. Reloading intentionally forgets the choice.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfirmedRunDisclosure {
    run_id: String,
    folded: bool,
    failed: bool,
}

/// Reconcile the one-off expansion with the latest host-scoped run snapshot.
///
/// Only a run with an id can own a fold choice. An idle, completed, or Update
/// snapshot clears it; a new id starts expanded, and failure overrides a fold.
fn confirmed_run_disclosure(
    prior: Option<&ConfirmedRunDisclosure>,
    view: Option<&ProvisioningView>,
) -> Option<ConfirmedRunDisclosure> {
    let view = view.filter(|view| {
        matches!(
            view.operation,
            Some(ProvisioningOperation::Add | ProvisioningOperation::Uninstall)
        ) && matches!(
            view.status,
            ProvisioningStatus::Running | ProvisioningStatus::Failed
        )
    })?;
    let run_id = view.run_id.as_ref()?.clone();
    let failed = view.status == ProvisioningStatus::Failed;
    Some(ConfirmedRunDisclosure {
        folded: !failed
            && prior
                .filter(|prior| prior.run_id == run_id)
                .is_some_and(|prior| prior.folded),
        run_id,
        failed,
    })
}

/// Derive the row-sized progress facts from the exact live UPDATE snapshot.
///
/// A mismatched, terminal, or non-UPDATE view cannot be presented as current
/// work. Unknown step statuses remain unfinished so a newer executor status
/// does not accidentally inflate the done count.
fn update_progress_summary(
    view: &ProvisioningView,
    tracked: &TrackedRun,
) -> Option<UpdateProgressSummary> {
    if tracked.operation != Some(ProvisioningOperation::Update)
        || tracked.run_id.is_empty()
        || view.run_id.as_deref() != Some(tracked.run_id.as_str())
        || view.operation != Some(ProvisioningOperation::Update)
        || view.status != ProvisioningStatus::Running
    {
        return None;
    }
    Some(summarize_running_view(
        view,
        tracked.run_id.clone(),
        tracked.started_at,
        ProvisioningOperation::Update,
    ))
}

/// The compact summary of one running snapshot: its steps, which one is
/// current, and how many are done. Callers have already established that
/// `view` is the live run they mean to summarize.
fn summarize_running_view(
    view: &ProvisioningView,
    run_id: String,
    started_at: Instant,
    operation: ProvisioningOperation,
) -> UpdateProgressSummary {
    let done = view
        .steps
        .iter()
        .filter(|step| {
            matches!(
                step.status.as_str(),
                "completed" | "skipped" | "degraded" | "failed"
            )
        })
        .count();
    let current_index = view.steps.iter().position(|step| step.status == "running");
    let current_step =
        current_index.map(|index| progress_step_name(&view.steps[index]).to_string());
    let steps = view
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| UpdateStepLine {
            id: step.step.clone(),
            name: progress_step_name(step).to_string(),
            status: step.status.clone(),
            // The popup's local snapshot still owns raw strings and escapes
            // them through PeerLine. Do not cache a display spelling here.
            message: step
                .message
                .as_ref()
                .map(|message| message.raw().to_owned()),
            current: Some(index) == current_index,
        })
        .collect();
    UpdateProgressSummary {
        run_id,
        operation,
        current_step,
        steps,
        done,
        total: view.steps.len(),
        started_at,
    }
}

/// The compact run status this panel publishes to its host row, if any.
///
/// A live automatic-update intent always wins and names its own phase, so
/// the click shows `updating…` from the moment it is accepted and never
/// falls back to the ordinary label while the POST is outstanding. After the
/// intent ends, only a live tracked UPDATE run keeps a status: full progress
/// when the displayed view is that exact run running, otherwise the
/// awaiting-progress stage. Confirmed setup and uninstall use a matching
/// observed run id and first-seen clock instead of Update's tracked run.
/// Terminal runs and rows with no current work return the ordinary label.
///
/// The same answer also decides whether the collapsed trace line is drawn:
/// while the row carries this inline status, a second trace line would only
/// repeat it.
fn derive_row_update_progress(
    intent: Option<&UpdateIntent>,
    tracked: Option<&TrackedRun>,
    view: Option<&ProvisioningView>,
    confirmed_run_clock: Option<&(String, Instant)>,
) -> Option<HostUpdateProgress> {
    if let Some(live) = intent {
        return Some(HostUpdateProgress::Pending(match live.phase {
            IntentPhase::Planning => UpdatePendingPhase::Planning,
            IntentPhase::WaitingForClaim => UpdatePendingPhase::WaitingForClaim,
            IntentPhase::Submitting => UpdatePendingPhase::Submitting,
        }));
    }
    if let Some(run) = tracked.filter(|run| {
        run.operation == Some(ProvisioningOperation::Update) && retained_run_live(Some(run))
    }) {
        return Some(
            view.and_then(|view| update_progress_summary(view, run))
                .map_or(
                    HostUpdateProgress::Pending(UpdatePendingPhase::AwaitingProgress),
                    HostUpdateProgress::Running,
                ),
        );
    }
    // A confirmed ADD or UNINSTALL has no tracked update run or intent. Its
    // live read is paired with the clock first observed for that exact id.
    let (clock_run, started_at) = confirmed_run_clock?;
    let view = view.filter(|view| {
        matches!(
            view.operation,
            Some(ProvisioningOperation::Add | ProvisioningOperation::Uninstall)
        ) && view.status == ProvisioningStatus::Running
            && view.run_id.as_deref() == Some(clock_run.as_str())
    })?;
    Some(HostUpdateProgress::Running(summarize_running_view(
        view,
        clock_run.clone(),
        *started_at,
        view.operation.expect("filtered to a confirmed operation"),
    )))
}

/// Whether a retained tracked run still owns its row.
///
/// Ownership runs from acceptance (or adoption) until a terminal observation:
/// Completed or Failed. A `None` last status is the gap between acceptance
/// and the first applicable read — still owned. An unrelated Completed view
/// never clears this; only the tracked run's own terminal state, vanishment
/// (which clears `tracked` outright), a retarget, or row removal ends it.
///
/// This does NOT filter by operation. The backend excludes overlap across
/// operations, so a live ADD observation blocks an update the same way a live
/// UPDATE does.
fn retained_run_live(tracked: Option<&TrackedRun>) -> bool {
    tracked.is_some_and(|run| {
        !matches!(
            run.last_status,
            Some(ProvisioningStatus::Completed | ProvisioningStatus::Failed)
        )
    })
}

/// Whether an automatic update currently owns this row.
///
/// A live intent in any phase, or a retained accepted/observed run that has
/// not reached a terminal state. The intent ends at acceptance; afterwards
/// the tracked run carries ownership until its own authoritative success, a
/// terminal observation, an explicitly diagnosed vanishment, a retarget, or
/// removal. Admission ([`decide_request`]), the menu, and busy publication
/// all derive from this, so an unrelated Completed view can neither re-offer
/// Update nor release the row's busy paint while the accepted run is still
/// retained. A terminal failure or a diagnosed vanishment ends ownership, and
/// deliberate retry becomes available again.
fn update_owned(intent: Option<&UpdateIntent>, tracked: Option<&TrackedRun>) -> bool {
    intent.is_some() || retained_run_live(tracked)
}

/// The opaque id and server-rendered text that must stay paired until POST.
///
/// Local confirmation and notice state retain raw strings: the plan renderer
/// escapes the confirmation and Copy plan needs its exact original bytes.
///
/// A displayed plan is the key of the row's confirmation [`ConfirmSlot`], so
/// the token an answer carries names the whole plan its render drew: the
/// `probe_id` the POST spends, the text and steps the user read, and the
/// binding the plan was made for.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingPlan {
    operation: ProvisioningOperation,
    binding: HostBinding,
    probe_id: String,
    confirmation: String,
    steps: Option<PlanSteps>,
    /// The data directory an UNINSTALL plan keeps, for the success notice;
    /// `None` for every other operation.
    kept_state_dir: Option<String>,
}

/// Retire only this host's mounted setup or uninstall question, without return.
///
/// A pending plan can still be waiting in HostModal while another host owns the
/// page. Its existing panel wrapper scopes release to the admitted question it
/// actually rendered; no mounted question means there is nothing to release.
/// Inline Update confirmation has no modal boundary and is deliberately absent.
fn release_plan_modal(host_id: HostId) {
    DialogHandle::new(r#".host-add-dialog[role="dialog"], .host-uninstall-dialog[role="dialog"]"#)
        .release_within(&format!(r#"[data-provisioning-host="{host_id}"]"#));
}

/// What answering a row's Setup, Uninstall or Update confirmation may do;
/// see [`take_plan_offer`].
#[derive(Debug, PartialEq)]
enum PlanConfirm<G> {
    /// The showing this answer was drawn for is no longer open (cancelled,
    /// already confirmed, retired by reconciliation, or replaced by another
    /// plan): nothing may happen, the "don't ask in the future" preference
    /// included, and whatever plan is open now stays up.
    Stale,
    /// The row's registry facts changed since the plan was made: the plan is
    /// withdrawn unsent, and the caller releases its modal.
    BindingChanged,
    /// Another operation holds the page: nothing happens, and the question
    /// stays up to be answered again.
    Refused,
    /// The showing is taken and the page lock claimed: the caller submits the
    /// plan its own token names (`shown.key()`), which is the one the
    /// answer's render drew.
    Accepted(G),
}

/// Confirm `shown`, the opening of the plan the answer was drawn for, against
/// the row's binding as it is now (`current`).
///
/// The answer acts on the plan `shown` names, never on whatever the row holds
/// at click time: an answer for a plan since replaced would otherwise spend a
/// `probe_id`, and carry out steps, the user never saw. So the plan travels
/// in the token, and the slot is only asked whether that showing is still
/// the open one.
///
/// Staleness is decided before `claim` runs, so an answer that can no longer
/// land never takes (and drops) the page lock, whose write would wake every
/// reader of the lock for nothing. A busy page refuses and leaves the
/// question up; the plan is taken only once the claim is held, so the claim
/// and the one-use plan move into the submission together.
///
/// The binding comparison is the row's long-standing check that a plan still
/// matches the host details it was made for ([`HostBinding`]: the entry and
/// its kind, its identity, its destination, its remote farhelm path, its
/// remote state directory and its connection incarnation). That comparison is
/// unchanged by moving these answers onto the showing; only its left-hand
/// side is now the rendered plan's binding instead of the live plan's, which
/// is the same plan once `is_current` has passed. No installation-identity
/// check was added: per SPEC.md ("Confirmed 2026-10-10") a confirmed host
/// operation follows the host entry, so nothing beyond that existing
/// comparison may be added here.
fn take_plan_offer<G>(
    slot: &mut ConfirmSlot<PendingPlan>,
    shown: &ConfirmToken<PendingPlan>,
    current: &HostBinding,
    claim: impl FnOnce() -> Option<G>,
) -> PlanConfirm<G> {
    if !slot.is_current(shown) {
        return PlanConfirm::Stale;
    }
    if shown.key().binding != *current {
        slot.cancel_for(shown);
        return PlanConfirm::BindingChanged;
    }
    let Some(claim) = claim() else {
        return PlanConfirm::Refused;
    };
    // Nothing between the check above and here can close the slot (this all
    // runs in one synchronous handler), but a miss still drops the claim
    // rather than submitting a plan nobody confirmed.
    if slot.take(shown).is_none() {
        return PlanConfirm::Stale;
    }
    PlanConfirm::Accepted(claim)
}

/// What preparing a run established without yet mutating the target.
enum Preparation {
    /// Keep the larger plan payload off the short refusal/discovery result path.
    Plan(Box<PendingPlan>),
    /// ADD found a supervisor and registered it as-is, so there is no run to
    /// confirm or follow.
    Discovered,
    /// The transport worked, but this target stays on the manual path.
    Manual(String),
    /// The probe may have registered a supervisor, but its successful reply
    /// did not say which outcome this client should render.
    Unvalidated(String),
}

/// The planning route's displayable failure and its trusted Remove remedy.
/// Only uninstall's typed response can set the remedy; other planning errors
/// carry prose alone, whether they came from the helm or from transport.
struct PreparationError {
    reason: String,
    remove_instead: bool,
}

impl From<String> for PreparationError {
    fn from(reason: String) -> Self {
        Self {
            reason,
            remove_instead: false,
        }
    }
}

impl From<crate::api::UninstallPlanError> for PreparationError {
    fn from(error: crate::api::UninstallPlanError) -> Self {
        Self {
            reason: error.reason,
            remove_instead: error.remove_instead,
        }
    }
}

/// Turn a failed explicit plan into the modal outcome for that action.
/// The local setup handoff is a state and stays in the row; no local Update
/// or uninstall plan can be offered.
fn planning_refusal(operation: ProvisioningOperation, error: PreparationError) -> HostRefusal {
    let action = match operation {
        ProvisioningOperation::Add => HostRefusalAction::Setup,
        ProvisioningOperation::Update => HostRefusalAction::Update,
        ProvisioningOperation::Uninstall => HostRefusalAction::Uninstall,
    };
    HostRefusal {
        action,
        reason: error.reason,
        remove_instead: error.remove_instead,
    }
}

/// A definite refusal of confirmed remote setup or uninstall reopens a
/// dialog. An ambiguous reply may have started a run and remains in the row;
/// one-click Update submission outcomes keep their row semantics too.
fn confirmed_submission_refusal(
    plan: &PendingPlan,
    error: &SubmissionError,
) -> Option<HostRefusal> {
    let SubmissionError::Refused(reason) = error else {
        return None;
    };
    if plan.operation == ProvisioningOperation::Update
        || (plan.operation == ProvisioningOperation::Add && plan.binding.kind.sets_up_locally())
    {
        return None;
    }
    Some(HostRefusal {
        action: if plan.operation == ProvisioningOperation::Uninstall {
            HostRefusalAction::Uninstall
        } else {
            HostRefusalAction::Setup
        },
        reason: reason.clone(),
        remove_instead: false,
    })
}

/// Peer text retains its original bytes for the renderer, but only its
/// displayed spelling contributes to an in-flow change. In particular,
/// different control-character spellings that escape identically do not
/// manufacture a new movement cause.
#[derive(Clone, Debug)]
struct FlowText(PeerText);

impl PartialEq for FlowText {
    fn eq(&self, other: &Self) -> bool {
        self.0.escaped() == other.0.escaped()
    }
}

impl Eq for FlowText {}

/// Keep peer isolation and authored wording intact while comparing exactly
/// the runs PeerLine displays. Flattening the sentence would lose its bidi
/// boundaries; comparing raw peer bytes would include invisible differences.
/// Mono and interface runs remain distinct because font metrics affect flow.
#[derive(Clone, Debug)]
struct FlowParts(Vec<DetailPart>);

impl PartialEq for FlowParts {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .zip(&other.0)
                .all(|(left, right)| match (left, right) {
                    (DetailPart::Text(left), DetailPart::Text(right)) => left == right,
                    (DetailPart::Peer(left), DetailPart::Peer(right))
                    | (DetailPart::MonoPeer(left), DetailPart::MonoPeer(right)) => {
                        left.escaped() == right.escaped()
                    }
                    _ => false,
                })
    }
}

impl Eq for FlowParts {}

/// A plan body contains the actual legacy/stepper render decision. Copy
/// bytes belong to its caller, and hover paths and list keys remain render
/// metadata rather than reasons to invalidate an anchored surface.
#[derive(Clone, Debug, PartialEq, Eq)]
enum PlanBody {
    // The renderer still receives raw bytes for its strict component props.
    // Movement uses PeerBlock's shared actual line body in `same_flow`.
    Legacy(PeerText),
    Steps {
        lead: FlowText,
        steps: Vec<PlanBodyStep>,
        kept: Option<FlowText>,
    },
}

impl PlanBody {
    /// Freeze the same body both inline and modal confirmations draw. The
    /// legacy renderer keeps raw line boundaries; structured plans ignore
    /// confirmation bytes here because those bytes are only used by Copy.
    fn new(confirmation: &str, steps: Option<&PlanSteps>) -> Self {
        let Some(steps) = steps else {
            return Self::Legacy(confirmation.into());
        };
        Self::Steps {
            lead: FlowText(steps.lead.clone()),
            steps: steps
                .steps
                .iter()
                .map(|step| PlanBodyStep {
                    id: step.id.clone(),
                    name: FlowText(step.name.clone()),
                    paths: step
                        .paths
                        .iter()
                        .map(|path| PlanBodyPath {
                            short: FlowText(path.short.clone()),
                            full: path.full.clone(),
                        })
                        .collect(),
                    explanation: step.explanation.clone().map(FlowText),
                })
                .collect(),
            kept: steps.kept.clone().map(FlowText),
        }
    }

    /// Dioxus props must retain strict equality so a changed tooltip or key
    /// reaches the renderer. Movement alone ignores that metadata; keeping
    /// this comparison separate avoids suppressing a legitimate prop update.
    /// Legacy text uses PeerBlock's actual displayed line structure, including
    /// CRLF normalization, editable escaping and literal final terminators.
    fn same_flow(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Legacy(left), Self::Legacy(right)) => {
                PeerBlockBody::new(left) == PeerBlockBody::new(right)
            }
            (
                Self::Steps {
                    lead: left_lead,
                    steps: left,
                    kept: left_kept,
                },
                Self::Steps {
                    lead: right_lead,
                    steps: right,
                    kept: right_kept,
                },
            ) => {
                left_lead == right_lead
                    && left_kept == right_kept
                    && left.len() == right.len()
                    && left.iter().zip(right).all(|(left, right)| {
                        left.name == right.name
                            && left.explanation == right.explanation
                            && left.paths.len() == right.paths.len()
                            && left
                                .paths
                                .iter()
                                .zip(&right.paths)
                                .all(|(left, right)| left.short == right.short)
                    })
            }
            _ => false,
        }
    }
}

/// A step's key preserves its existing DOM lifetime without becoming a
/// content-change cause. Only the name, displayed paths and explanation
/// occupy the plan's ordinary flow.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PlanBodyStep {
    id: String,
    name: FlowText,
    paths: Vec<PlanBodyPath>,
    explanation: Option<FlowText>,
}

/// The full path is hover-only. Strict prop equality retains its updates;
/// `same_flow` compares only the short path actually drawn in flow.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PlanBodyPath {
    short: FlowText,
    full: PeerText,
}

/// Show the helm's ordered actions without reconstructing their wording.
///
/// Each path uses the helm's short spelling in the narrow dialog and exposes
/// its full spelling on hover. Copy uses the untouched confirmation string:
/// escaped display text and separate chips cannot reproduce those bytes by
/// native selection. Older replies without steps retain the text renderer.
#[component]
fn PlanStepList(confirmation: String, body: PlanBody) -> Element {
    let (lead, steps, kept) = match body {
        PlanBody::Legacy(text) => {
            return rsx! { PeerBlock { class: "provisioning-plan", text } };
        }
        PlanBody::Steps { lead, steps, kept } => (lead, steps, kept),
    };
    rsx! {
        div { class: "provisioning-plan provisioning-stepper-plan",
            div { class: "provisioning-plan-heading",
                PeerLine {
                    class: "provisioning-plan-lead",
                    parts: vec![DetailPart::peer(lead.0)],
                }
                button {
                    r#type: "button",
                    class: "btn btn-neutral provisioning-copy-plan",
                    "data-tooltip": "copy the complete host plan",
                    onclick: move |_| crate::session_view::copy_to_clipboard(&confirmation),
                    "copy plan"
                }
            }
            ol { class: "provisioning-plan-steps",
                for (index, step) in steps.into_iter().enumerate() {
                    li { key: "{step.id}", class: "provisioning-plan-step",
                        span { class: "provisioning-plan-number", aria_hidden: "true", "{index + 1}" }
                        div { class: "provisioning-plan-detail",
                            PeerLine {
                                class: "provisioning-plan-step-name",
                                parts: vec![DetailPart::peer(step.name.0)],
                            }
                            if !step.paths.is_empty() {
                                div { class: "provisioning-plan-paths",
                                    for path in step.paths {
                                        span {
                                            class: "provisioning-plan-path peer-value",
                                            dir: "ltr",
                                            "data-tooltip": "{path.full.escaped()}",
                                            "{path.short.0.escaped()}"
                                        }
                                    }
                                }
                            }
                            if let Some(explanation) = step.explanation {
                                PeerLine {
                                    class: "provisioning-plan-explanation",
                                    parts: vec![DetailPart::peer(explanation.0)],
                                }
                            }
                        }
                    }
                }
            }
            if let Some(kept) = kept {
                PeerLine {
                    class: "provisioning-plan-kept",
                    parts: vec![DetailPart::peer(kept.0)],
                }
            }
        }
    }
}

/// Present a helm-authored plan through the shared stepper.
///
/// This component deliberately does not walk `ProvisioningPlan.actions`.
/// The server's steps and copyable text come from the same frozen actions the
/// run consumes. This defensive update path shares the setup presentation so
/// it cannot invent different promises for the same action.
#[component]
fn PlanConfirmation(
    confirmation: String,
    body: PlanBody,
    busy: bool,
    confirm_label: &'static str,
    on_confirm: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "provisioning-confirmation",
            PlanStepList { confirmation, body }
            div { class: "provisioning-confirm-actions",
                button {
                    r#type: "button",
                    class: "btn btn-primary provisioning-confirm",
                    "data-tooltip": "{confirm_label}: carry out the plan shown above",
                    disabled: busy,
                    autofocus: !busy,
                    onkeydown: crate::guard_confirmation_enter,
                    onmounted: move |_| crate::focus_confirmation(".provisioning-confirmation .provisioning-confirm", ".provisioning-confirmation .provisioning-cancel"),
                    onclick: move |_| on_confirm.call(()),
                    "{confirm_label}"
                }
                button {
                    r#type: "button",
                    class: "btn btn-neutral provisioning-cancel",
                    "data-tooltip": "cancel: change nothing on this host",
                    // Retiring an unsubmitted offer cannot affect the page
                    // token held by a different operation. Keep this fallback live.
                    autofocus: busy,
                    onclick: move |_| on_cancel.call(()),
                    "cancel"
                }
            }
        }
    }
}

/// Render the three answers for an initial or rerun host setup offer.
///
/// Both callers place these header, body and footer siblings directly in the
/// host dialog frame; a very short window scrolls the whole card instead of
/// pinning chrome that cannot fit. The helm supplies the plan;
/// this component owns presentation, never its actions or authorization. The
/// caller supplies an already escaped display name and its cancel lifecycle.
/// Text-only plans need a named entry stop in WebKit. Structured plans already
/// provide entry through Copy Plan; the shared scroll boundary distinguishes them.
///
/// `busy` disables only the two answers that would submit. Cancel stays
/// live: this renders inside a modal, and a modal must never hold the user
/// behind somebody else's pending request.
#[component]
pub(crate) fn SetupPlanConfirmation(
    confirmation: String,
    steps: Option<PlanSteps>,
    name: String,
    host: Option<Host>,
    local: bool,
    error: Option<Element>,
    busy: bool,
    on_confirm: EventHandler<bool>,
    on_cancel: EventHandler<()>,
) -> Element {
    let body = PlanBody::new(&confirmation, steps.as_ref());
    rsx! {
        settings_dialog::HostDialogHeader {
            host,
            title: rsx! { "set up host · " span { class: "peer-value", dir: "ltr", "{name}" } },
            summary: if local { "Farhelm is not running here yet" } else { "Farhelm is not running there yet" },
            dismiss_label: "cancel host setup",
            dismiss_tooltip: "cancel: change nothing on this host",
            on_dismiss: on_cancel,
        }
        ScrollRegion { class: "dialog-body host-settings-body",
            handle: settings_dialog::ADD_DIALOG, selector: ".host-settings-body", name: "host setup plan",
            recovery: RecoveryFocus::new(".add-host-cancel", None, true, RecoveryDelay::OneFrame),
            PlanStepList { confirmation, body }
            if let Some(error) = error { {error} }
        }
        crate::modal_dialog::DialogFooter { class: "host-dialog-footer",
            p { id: "host-setup-permanent-hint", class: "dialog-footer-hint host-dialog-permanent-hint",
                "undo \"don't ask again\" with the gear at the top of the sidebar"
            }
            crate::modal_dialog::DialogActions { class: "add-host-confirm-actions",
            button {
                r#type: "button",
                class: "btn btn-neutral add-host-cancel",
                "data-tooltip": "cancel: change nothing on this host",
                autofocus: busy,
                onclick: move |_| on_cancel.call(()),
                "cancel"
            }
            button {
                r#type: "button",
                class: "btn btn-primary btn-outline host-setup-permanent",
                "data-tooltip": "set up this host, and set up new hosts from now on without asking",
                aria_describedby: "host-setup-permanent-hint",
                disabled: busy,
                onkeydown: crate::guard_confirmation_enter,
                onclick: move |_| on_confirm.call(true),
                "set up, don't ask again"
            }
            button {
                r#type: "button",
                class: "btn btn-primary provisioning-confirm",
                "data-tooltip": "set up: set up this host as the plan above describes",
                disabled: busy,
                autofocus: !busy,
                onkeydown: crate::guard_confirmation_enter,
                onclick: move |_| on_confirm.call(false),
                "set up"
            }
            }
        }
    }
}

/// Render the helm's uninstall plan with its two answers.
///
/// The helm-authored steps name what goes and the separate note names what
/// stays. The confirm button is in the danger tier
/// because the removal cannot be undone from Farhelm. Confirm takes initial
/// focus when enabled so a new Enter accepts the requested plan. These frame siblings
/// normally pin the host identity and answers around the scrollable plan; a
/// very short window scrolls the whole card to keep every part reachable. Header
/// dismissal uses the caller's Cancel path, including its row-focus return.
/// Copy Plan provides keyboard scroll entry when present; an older text-only
/// plan gets a named body stop without moving answers that still fit pinned.
#[component]
pub(crate) fn UninstallConfirmation(
    confirmation: String,
    steps: Option<PlanSteps>,
    host: Host,
    busy: bool,
    on_confirm: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let name = crate::hosts::gui_host_name(host.name.raw(), host.kind == HostKind::Local);
    let body = PlanBody::new(&confirmation, steps.as_ref());
    rsx! {
        settings_dialog::HostDialogHeader {
            host: Some(host),
            title: rsx! { "uninstall Farhelm · " span { class: "peer-value", dir: "ltr", "{name}" } },
            dismiss_label: "cancel Farhelm uninstall",
            dismiss_tooltip: "cancel: leave Farhelm installed on this host",
            on_dismiss: on_cancel,
        }
        ScrollRegion { class: "dialog-body host-settings-body",
            handle: DialogHandle::new(settings_dialog::UNINSTALL_DIALOG_SELECTOR), selector: ".host-settings-body", name: "host uninstall plan",
            recovery: RecoveryFocus::new(".uninstall-cancel", None, true, RecoveryDelay::OneFrame),
            PlanStepList { confirmation, body }
        }
        crate::modal_dialog::DialogFooter { class: "host-dialog-footer",
            crate::modal_dialog::DialogActions {
                button {
                    r#type: "button",
                    class: "btn btn-neutral provisioning-cancel uninstall-cancel",
                    "data-tooltip": "cancel: leave Farhelm installed on this host",
                    autofocus: busy,
                    onclick: move |_| on_cancel.call(()),
                    "cancel"
                }
                button {
                    r#type: "button",
                    class: "btn btn-danger provisioning-confirm uninstall-confirm",
                    "data-tooltip": "uninstall: remove Farhelm from this host as described above",
                    disabled: busy,
                    autofocus: !busy,
                    onkeydown: crate::guard_confirmation_enter,
                    onclick: move |_| on_confirm.call(()),
                    "uninstall"
                }
            }
        }
    }
}

/// Prepare a fresh plan for the requested idempotent operation.
/// Only uninstall's planning error can carry the helm's typed Remove remedy;
/// all other errors preserve their display text without classifying status.
async fn prepare(
    base: &str,
    host: &Host,
    operation: ProvisioningOperation,
) -> Result<Preparation, PreparationError> {
    let binding = HostBinding::from(host);
    match operation {
        ProvisioningOperation::Update => {
            let planned = plan_host_update(base, host.id).await?;
            Ok(Preparation::Plan(Box::new(PendingPlan {
                operation,
                binding,
                probe_id: planned.probe_id,
                confirmation: planned.confirmation.raw().to_owned(),
                steps: planned.steps,
                kept_state_dir: None,
            })))
        }
        ProvisioningOperation::Uninstall => {
            let planned = plan_host_uninstall(base, host.id).await?;
            Ok(Preparation::Plan(Box::new(PendingPlan {
                operation,
                binding,
                probe_id: planned.probe_id,
                confirmation: planned.confirmation.raw().to_owned(),
                steps: planned.steps,
                // The success notice owns a local raw path for clipboard copy;
                // its visible and accessible labels still use peer rendering.
                kept_state_dir: Some(planned.plan.state_dir.raw().to_owned()),
            })))
        }
        ProvisioningOperation::Add => {
            let probed = match host.kind {
                HostKind::Local => probe_local_host(base).await?,
                HostKind::Ssh => {
                    let destination = host
                        .destination
                        .as_ref()
                        .map(|value| value.raw())
                        .ok_or_else(|| "this ssh host has no destination to probe".to_string())?;
                    probe_ssh_host(
                        base,
                        destination,
                        host.remote_farhelm
                            .as_ref()
                            .map(|value| value.raw())
                            .unwrap_or_default(),
                        host.remote_state_dir
                            .as_ref()
                            .map(|value| value.raw())
                            .unwrap_or_default(),
                        None,
                    )
                    .await?
                }
                HostKind::Unrecognized => {
                    return Err(
                        "this build does not recognize the host kind, so it cannot provision it"
                            .to_string()
                            .into(),
                    );
                }
            };
            Ok(match probed {
                ProbeResponse::Discovered => Preparation::Discovered,
                ProbeResponse::Provisionable {
                    probe_id,
                    confirmation,
                    steps,
                } => Preparation::Plan(Box::new(PendingPlan {
                    operation,
                    binding,
                    probe_id,
                    confirmation: confirmation.raw().to_owned(),
                    steps,
                    kept_state_dir: None,
                })),
                // The local diagnostic state shares transport-error strings;
                // every presentation of this state still enters PeerLine.
                ProbeResponse::Manual { reason } => Preparation::Manual(reason.raw().to_owned()),
                ProbeResponse::Unvalidated(problem) => Preparation::Unvalidated(problem),
            })
        }
    }
}

/// Everything the one shared submission body needs beyond the plan itself.
///
/// Both explicit setup confirmation and automatic remote update enter
/// [`submit_plan`]; a rendered button is never part of the mechanism. All
/// handles are `Copy`, so this moves into the spawned POST task whole.
struct Submission<R> {
    base: String,
    host_id: HostId,
    /// The row can disappear before a definite reply arrives, but its
    /// refusal dialog still needs the identity the user acted on.
    host_snapshot: Host,
    /// The submitting update intent's epoch, or `None` for an explicit ADD
    /// confirmation. Correlates the outcome with the intent it ends.
    submitted_epoch: Option<u64>,
    claim: OpGuard,
    current_binding: Memo<HostBinding>,
    intent: Signal<Option<UpdateIntent>>,
    /// The in-flight update POST, independent of the invalidatable intent.
    /// Updates set this at claim and clear it when the outcome installs; ADD
    /// (which has no intent and no suppression) passes the handle through
    /// untouched. The read path suppresses progress commits while this
    /// stands, even after a retarget drops the intent.
    outstanding: Signal<Option<OutstandingSubmission>>,
    tracked: Signal<Option<TrackedRun>>,
    /// Run ids a target change detached from this row. Acceptance
    /// reconciliation writes here (never reads): a late acceptance whose
    /// submitting target is gone retires its old-target run before the
    /// follow-up reread, so that read cannot adopt it as new-target work.
    retired_runs: Signal<HashSet<String>>,
    read_gate: Signal<ReadGate>,
    action_error: Signal<Option<String>>,
    action_warning: Signal<Option<String>>,
    update_error: Signal<Option<UpdateDiagnostic>>,
    update_warning: Signal<Option<UpdateDiagnostic>>,
    accepted_run_waiting: Signal<bool>,
    local_auto_retry: Signal<bool>,
    on_running: EventHandler<bool>,
    on_changed: EventHandler<()>,
    /// The panel owns this notice beyond the row's lifetime. A definite
    /// uninstall refusal withdraws it even when another window removed the
    /// row while the submission reply was in flight.
    uninstall_notices: Signal<HashMap<HostId, UninstallNotice>>,
    /// Definite setup-rerun and uninstall refusals reopen as modal outcomes;
    /// Update submissions keep their existing row diagnostic.
    refusals: Signal<HashMap<HostId, HostRefusalEntry>>,
    reread: R,
}

/// Release one POST's progress suppression once its outcome has installed.
///
/// Epoch-guarded: only the submission that raised suppression may lower it,
/// so a newer POST's outstanding marker is never cleared by an older outcome
/// landing late.
fn release_outstanding(
    outstanding: &mut Signal<Option<OutstandingSubmission>>,
    submitted_epoch: u64,
) {
    if outstanding
        .peek()
        .as_ref()
        .is_some_and(|held| held.epoch == submitted_epoch)
    {
        outstanding.set(None);
    }
}

/// Submit one binding-checked plan exactly once, retaining its reply if the
/// host row disappears before the helm answers.
///
/// The caller has already validated the plan against the live binding,
/// claimed the page token, and removed the plan from client state, so every
/// path below keeps the token spent: nothing here replans, resubmits, or
/// replays. The page guard drops as soon as the POST has an outcome —
/// accepted, accepted-but-unreadable, or refused — before any follow-up
/// GET; progress from there is reads. Reads completing during the interval
/// commit nothing (the read path suppresses them and keeps retry demand
/// alive); a [`ReadGate`] fence at install rejects every read that
/// overlapped submission, and a fresh read follows.
///
/// ADD keeps its historical reconciliation: the returned row is the route
/// (registration can legitimately hand back an existing row), and an
/// accepted-but-unreadable body paints busy until reads say otherwise.
/// UPDATE is host-bound instead: a wrong-host acceptance keeps a
/// source-row protocol warning and reconciles without claiming success.
/// The POST task outlives this row; after it resolves, only parent-owned
/// refusal and refresh state may be touched if the row was removed.
fn submit_plan<R>(submission: Submission<R>, plan: PendingPlan)
where
    R: Fn(Trigger) + Clone + 'static,
{
    let Submission {
        base,
        host_id,
        host_snapshot,
        submitted_epoch,
        claim,
        current_binding,
        mut intent,
        mut outstanding,
        mut tracked,
        mut retired_runs,
        mut read_gate,
        mut action_error,
        mut action_warning,
        mut update_error,
        mut update_warning,
        mut accepted_run_waiting,
        mut local_auto_retry,
        on_running,
        on_changed,
        mut uninstall_notices,
        mut refusals,
        reread,
    } = submission;
    dioxus::core::spawn_forever(async move {
        let result = match plan.operation {
            ProvisioningOperation::Add => provision_host(&base, &plan.probe_id).await,
            ProvisioningOperation::Update => update_host(&base, host_id, &plan.probe_id).await,
            ProvisioningOperation::Uninstall => {
                uninstall_host(&base, host_id, &plan.probe_id).await
            }
        };
        // The task survives its row so a concurrent removal cannot eat a
        // definite reply. It also survives navigation away from the hosts
        // panel; in that case there is no UI owner left to receive it.
        drop(claim);
        if refusals.try_peek().is_err() {
            return;
        }
        if plan.operation == ProvisioningOperation::Uninstall
            && matches!(result, Err(SubmissionError::Refused(_)))
        {
            uninstall_notices.write().remove(&host_id);
        }
        // A confirmed POST can outlive its host row (another window may
        // remove that host while this reply is in flight). The parent owns
        // refusals and the registry refresh; the row's signals have already
        // been disposed, so only those parent-owned outcomes survive here.
        if current_binding.try_peek().is_err() {
            if let Err(error) = result
                && let Some(refusal) = confirmed_submission_refusal(&plan, &error)
            {
                refusals
                    .write()
                    .insert(host_id, HostRefusalEntry::new(&host_snapshot, refusal));
            }
            on_changed.call(());
            return;
        }
        // Reads overlapping submission describe the pre-submission world.
        // Reject them before installing anything, then read fresh.
        read_gate.write().fence();
        match result {
            Ok(ProvisioningSubmission::Accepted(accepted)) => {
                submit_accepted(
                    &base,
                    host_id,
                    &plan,
                    submitted_epoch,
                    accepted,
                    current_binding,
                    &mut intent,
                    &mut outstanding,
                    &mut tracked,
                    &mut retired_runs,
                    &mut action_warning,
                    &mut update_warning,
                    &mut accepted_run_waiting,
                    on_running,
                    on_changed,
                    &reread,
                )
                .await;
            }
            Ok(ProvisioningSubmission::Unvalidated(warning)) => {
                match submitted_epoch {
                    Some(epoch) => {
                        // The 202 committed but left no run identity to
                        // follow. Keep the uncertainty visible and let
                        // reads drive busy paint if a run shows up; claim
                        // no run of our own.
                        if intent
                            .peek()
                            .as_ref()
                            .is_some_and(|live| live.epoch == epoch)
                        {
                            intent.set(None);
                        }
                        update_warning.set(Some(UpdateDiagnostic {
                            epoch,
                            sticky: true,
                            host_changed: false,
                            text: warning,
                        }));
                        release_outstanding(&mut outstanding, epoch);
                    }
                    None => {
                        accepted_run_waiting.set(true);
                        on_running.call(true);
                        action_warning.set(Some(warning));
                    }
                }
                reread(Trigger::Explicit);
                on_changed.call(());
            }
            Err(error) => {
                match submitted_epoch {
                    Some(epoch) => {
                        if intent
                            .peek()
                            .as_ref()
                            .is_some_and(|live| live.epoch == epoch)
                        {
                            intent.set(None);
                        }
                        // Either way this attempt ends with a fresh plan owed
                        // to the next one, never a replay of this token. But
                        // only a refusal describes this attempt: a lost reply
                        // may have committed, so its uncertainty is sticky —
                        // a new attempt's click, planning failure, or definite
                        // refusal must not clear it, only a correlated success
                        // or removal.
                        match error {
                            SubmissionError::Refused(text) => {
                                // A definite refusal proves this attempt
                                // submitted nothing, so an earlier sticky
                                // uncertainty outranks it: preserve the held
                                // diagnostic and its epoch (see
                                // `set_planning_diagnostic`). Ordinary refusals
                                // still install when nothing sticky stands.
                                if !update_error.peek().as_ref().is_some_and(|held| held.sticky) {
                                    update_error.set(Some(UpdateDiagnostic {
                                        epoch,
                                        sticky: false,
                                        host_changed: false,
                                        text,
                                    }));
                                }
                            }
                            SubmissionError::Ambiguous(error) => {
                                update_error.set(Some(UpdateDiagnostic {
                                    epoch,
                                    sticky: true,
                                    host_changed: false,
                                    text: format!(
                                        "the update was submitted but its reply was lost ({error}); \
                                         the run may have started, so its outcome is unknown"
                                    ),
                                }));
                            }
                        }
                        release_outstanding(&mut outstanding, epoch);
                    }
                    None => {
                        if plan.binding.kind.sets_up_locally()
                            && plan.operation == ProvisioningOperation::Add
                        {
                            local_auto_retry.set(true);
                        }
                        if let Some(refusal) = confirmed_submission_refusal(&plan, &error) {
                            refusals
                                .write()
                                .insert(host_id, HostRefusalEntry::new(&host_snapshot, refusal));
                        } else {
                            action_error.set(Some(error.into_text()));
                        }
                    }
                }
                reread(Trigger::Explicit);
                on_changed.call(());
            }
        }
    });
}

/// Reconcile one decoded acceptance: route ADD, bind UPDATE to its row.
///
/// `submit_plan`'s accepted arm, split out because the two operations answer
/// a wrong-host reply oppositely and the branch deserves its own contract.
/// ADD registration can legitimately return an existing row, so the accepted
/// id is the route. `start_update` is host-bound, so for UPDATE anything but
/// this row's own id — or an empty run id that could never settle — keeps a
/// source-row warning and reconciles without tracking a run.
#[allow(clippy::too_many_arguments)]
async fn submit_accepted<R>(
    base: &str,
    host_id: HostId,
    plan: &PendingPlan,
    submitted_epoch: Option<u64>,
    accepted: ProvisioningAccepted,
    current_binding: Memo<HostBinding>,
    intent: &mut Signal<Option<UpdateIntent>>,
    outstanding: &mut Signal<Option<OutstandingSubmission>>,
    tracked: &mut Signal<Option<TrackedRun>>,
    retired_runs: &mut Signal<HashSet<String>>,
    action_warning: &mut Signal<Option<String>>,
    update_warning: &mut Signal<Option<UpdateDiagnostic>>,
    accepted_run_waiting: &mut Signal<bool>,
    on_running: EventHandler<bool>,
    on_changed: EventHandler<()>,
    reread: &R,
) where
    R: Fn(Trigger),
{
    let Some(epoch) = submitted_epoch else {
        // Paint the returned row busy before its follow-up GET
        // can land. ADD registration can legitimately return an
        // existing row, so the accepted id is the route, not the
        // panel that happened to submit it.
        on_changed.call(());
        if accepted.host_id == host_id {
            accepted_run_waiting.set(true);
            on_running.call(true);
            reread(Trigger::Explicit);
        } else {
            // The returned row owns its paint-only busy bit; its
            // feed-driven reader will publish that state. This
            // immediate read still catches a run that cannot be
            // reconciled after acceptance.
            if let Err(error) = fetch_provisioning(base, accepted.host_id).await
                && current_binding.try_peek().is_ok()
            {
                action_warning.set(Some(format!(
                    "the helm accepted run {} for host {}, but its progress could not yet be read: {}",
                    accepted.run_id, accepted.host_id, error
                )));
            }
        }
        return;
    };
    if intent
        .peek()
        .as_ref()
        .is_some_and(|live| live.epoch == epoch)
    {
        intent.set(None);
    }
    // A newer intent may already own the row (this POST flew while the row
    // retargeted and the user clicked again). The outcome still installs as
    // late evidence, but it never clears that intent or its diagnostics.
    // Owned snapshot, not a borrowed peek: guards must not outlive the
    // statement that takes them (see the consumption closure's comment).
    let current = current_binding.peek().clone();
    let submitted_binding = outstanding
        .peek()
        .as_ref()
        .filter(|submission| submission.epoch == epoch)
        .map_or_else(
            || plan.binding.clone(),
            |submission| submission.binding.clone(),
        );
    if !same_update_target(&submitted_binding, &current) {
        // The accepted run belongs to the old target, but the progress
        // route is host-scoped: without this, the reread below adopts the
        // old run as the new target's history (the binding watcher only
        // retires a run that was already tracked, and nothing was). Retire
        // the validated id first, before suppression releases: a read that
        // completes after the release must already find this id retired.
        // Only a same-host, nonempty id retires — a wrong-host or empty
        // acceptance names nothing this row's reads could mistake for
        // their own, and must not poison the retired set.
        if accepted.host_id == host_id && !accepted.run_id.is_empty() {
            retired_runs.write().insert(accepted.run_id.clone());
            // A read may have adopted this id before suppression engaged
            // (or on a thread beside this install): same-ID reconciliation
            // ignores retirement, so detach the exact id here. Anything
            // else tracked is newer unrelated work and stays.
            if tracked
                .peek()
                .as_ref()
                .is_some_and(|old| old.run_id == accepted.run_id)
            {
                tracked.set(None);
                on_running.call(false);
            }
        }
        let warning = changed_host_diagnostic(
            update_warning.peek().as_ref(),
            epoch,
            &submitted_binding,
            &current,
            true,
        );
        if let Some(warning) = warning {
            update_warning.set(Some(warning));
        }
        release_outstanding(outstanding, epoch);
        reread(Trigger::Explicit);
        on_changed.call(());
        return;
    }
    if accepted.host_id != host_id {
        update_warning.set(Some(UpdateDiagnostic {
            epoch,
            sticky: true,
            host_changed: false,
            text: format!(
                "the helm accepted this update for host {} instead of this row; the plan is spent \
                 and this row claims nothing from that run",
                accepted.host_id
            ),
        }));
        release_outstanding(outstanding, epoch);
        reread(Trigger::Explicit);
        on_changed.call(());
        return;
    }
    if accepted.run_id.is_empty() {
        update_warning.set(Some(UpdateDiagnostic {
            epoch,
            sticky: true,
            host_changed: false,
            text: "the helm accepted the update but returned no run identity, so its outcome is \
                 unknown; this row claims nothing from that run"
                .to_string(),
        }));
        release_outstanding(outstanding, epoch);
        reread(Trigger::Explicit);
        on_changed.call(());
        return;
    }
    // The acceptance is authoritative, but a different run followed until
    // now ended unseen when this one was admitted (the backend excludes
    // overlap): a foreign run that finished between observation and
    // acceptance. Its uncertainty stays visible beside the new run.
    if let Some(prior) = tracked.peek().clone()
        && prior.run_id != accepted.run_id
        && prior.operation == Some(ProvisioningOperation::Update)
        && !matches!(
            prior.last_status,
            Some(ProvisioningStatus::Completed | ProvisioningStatus::Failed)
        )
    {
        update_warning.set(Some(UpdateDiagnostic {
            epoch,
            sticky: true,
            host_changed: false,
            text: format!(
                "update run {} ended without reporting its outcome; following the accepted run instead",
                prior.run_id
            ),
        }));
    }
    tracked.set(Some(TrackedRun {
        host: host_id,
        run_id: accepted.run_id,
        operation: Some(ProvisioningOperation::Update),
        source: RunSource::Submitted { epoch },
        // Acceptance may be the first time the row knows its identity.
        // Anchor that checked identity now rather than retaining the plan's
        // unknown identity and allowing a later replacement to pass too.
        binding: current,
        last_status: None,
        started_at: Instant::now(),
    }));
    on_running.call(true);
    release_outstanding(outstanding, epoch);
    reread(Trigger::Explicit);
    on_changed.call(());
}

/// The row state one applicable progress read may settle.
///
/// Grouped so [`reconcile_progress`] takes the read and its commit surface
/// instead of a dozen loose handles. The caller has already passed the
/// read gate: this runs only for reads the current lifecycle still owns.
struct ProgressCommit<'a> {
    host_id: HostId,
    binding: HostBinding,
    epoch_now: u64,
    retired: HashSet<String>,
    intent: &'a mut Signal<Option<UpdateIntent>>,
    epoch: &'a mut Signal<u64>,
    tracked: &'a mut Signal<Option<TrackedRun>>,
    /// The row's plan confirmation, retired (never answered) when a run is
    /// observed.
    offer: &'a mut ConfirmSlot<PendingPlan>,
    accepted_run_waiting: &'a mut Signal<bool>,
    update_error: &'a mut Signal<Option<UpdateDiagnostic>>,
    update_warning: &'a mut Signal<Option<UpdateDiagnostic>>,
    progress: &'a mut Signal<Option<ProvisioningView>>,
    progress_error: &'a mut Signal<Option<String>>,
    on_running: EventHandler<bool>,
}

/// The non-sticky explanation a competing run would leave, unless a sticky
/// uncertainty already stands.
///
/// A competing run proves this attempt submitted nothing, while a held
/// sticky warning means an earlier submission may have committed with its
/// outcome still unknown. The uncertainty outranks the explanation: the
/// caller keeps invalidation and run adoption, but installs nothing and
/// leaves the held diagnostic — epoch and text — untouched. Only a
/// correlated success or removal clears submission uncertainty; the
/// separate connected/version rule may clear change-of-host advice.
fn competing_run_warning(
    held: Option<&UpdateDiagnostic>,
    epoch: u64,
    text: String,
) -> Option<UpdateDiagnostic> {
    if held.is_some_and(|held| held.sticky) {
        return None;
    }
    Some(UpdateDiagnostic {
        epoch,
        sticky: false,
        host_changed: false,
        text,
    })
}

/// Invalidate an unsubmitted intent for an observed running run, and follow it.
///
/// Shared by the progress-read path and the claim effect's competing-run
/// recheck (a read that raced the wait). Bumps the epoch so in-flight
/// planning cannot restore the plan, drops the plan unsent, adopts the
/// observed run, and leaves the explanation that the click did not submit —
/// unless a sticky submission uncertainty already stands, which it preserves
/// (see [`competing_run_warning`]).
/// Never queues an extra update behind the observed run.
#[allow(clippy::too_many_arguments)]
fn invalidate_intent_for_run(
    view: &ProvisioningView,
    host_id: HostId,
    binding: &HostBinding,
    retired: &HashSet<String>,
    intent: &mut Signal<Option<UpdateIntent>>,
    epoch: &mut Signal<u64>,
    tracked: &mut Signal<Option<TrackedRun>>,
    update_warning: &mut Signal<Option<UpdateDiagnostic>>,
) {
    let counter = *epoch.peek() + 1;
    epoch.set(counter);
    intent.set(None);
    let run_id = view.run_id.clone().unwrap_or_default();
    // A retired run id is old-target evidence a retarget detached: the
    // intent still dies (nothing submits blind), but the run is not
    // adopted as the new target's history.
    if retired.contains(&run_id) {
        // Owned snapshot: a peek guard borrowed into the helper below would
        // stay alive across the `set` in its arm and panic the wasm runtime.
        let held = update_warning.peek().clone();
        if let Some(warning) = competing_run_warning(
            held.as_ref(),
            counter,
            "another provisioning run is already active on this host, so this update was \
                 not submitted"
                .to_string(),
        ) {
            update_warning.set(Some(warning));
        }
        return;
    }
    let previous = tracked.peek().clone();
    let started_at = previous
        .as_ref()
        .filter(|old| old.run_id == run_id)
        .map_or_else(Instant::now, |old| old.started_at);
    let ours = previous.as_ref().is_some_and(|old| old.run_id == run_id);
    tracked.set(Some(TrackedRun {
        host: host_id,
        run_id,
        operation: view.operation,
        source: RunSource::Observed,
        binding: binding.clone(),
        last_status: Some(ProvisioningStatus::Running),
        started_at,
    }));
    // Owned snapshot, as in the retired branch above.
    let held = update_warning.peek().clone();
    if let Some(warning) = competing_run_warning(
        held.as_ref(),
        counter,
        if ours {
            "an update is already running on this host, so the extra update was not submitted"
                .to_string()
        } else {
            "another provisioning run started on this host first, so this update was not \
             submitted; following the running run instead"
                .to_string()
        },
    ) {
        update_warning.set(Some(warning));
    }
}

/// Settle one applicable host-scoped progress read against update state.
///
/// Four questions in order, each defensive against the others' races:
///
/// - A run observed while an update plan is still unsubmitted (planning or
///   waiting for the claim) invalidates that intent — with an epoch bump so
///   the in-flight planning future cannot restore it — and follows the
///   observed run with an explanation. Nothing queues behind a foreign run.
///   A POST already in flight is left to its own outcome, which reconciles
///   (usually as a Busy refusal) when it lands; reads completing while that
///   POST is outstanding never reach this function — the read path
///   suppresses them, keyed on [`OutstandingSubmission`] rather than the
///   intent (a retarget can drop the intent mid-POST), and keeps retry
///   demand alive instead.
/// - A running or failed UPDATE with no tracked run is adopted: another
///   client's run, or retained failure visibility on mount. Completed
///   history alone adopts nothing; a resting row stays resting.
/// - A view naming a different run than the tracked one proves the tracked
///   run ended unseen (the backend excludes overlap), unless its terminal
///   state was already observed — and then only the sticky
///   unknown-outcome diagnostic remains, never a guessed resolution.
/// - Only [`authoritative_update_success`] for the exact tracked run id
///   settles it, clearing diagnostics at or below its submitting epoch. An
///   older run's success, an idle view, or a bare successful GET settles
///   nothing and clears no diagnostic.
fn reconcile_progress(view: ProvisioningView, commit: ProgressCommit<'_>) {
    let ProgressCommit {
        host_id,
        binding,
        epoch_now,
        retired,
        intent,
        epoch,
        tracked,
        offer,
        accepted_run_waiting,
        update_error,
        update_warning,
        progress,
        progress_error,
        on_running,
    } = commit;
    let running = view.run_id.is_some() && view.status == ProvisioningStatus::Running;

    // A competing run while this client's plan is still unsubmitted.
    let pre_submission = intent.peek().as_ref().is_some_and(|live| {
        matches!(
            live.phase,
            IntentPhase::Planning | IntentPhase::WaitingForClaim
        )
    });
    if pre_submission && running {
        invalidate_intent_for_run(
            &view,
            host_id,
            &binding,
            &retired,
            intent,
            epoch,
            tracked,
            update_warning,
        );
    } else {
        reconcile_tracked(
            &view,
            host_id,
            &binding,
            epoch_now,
            &retired,
            tracked,
            update_error,
            update_warning,
        );
    }

    if running {
        // A run observed from another browser supersedes any ADD plan this
        // component was still displaying. (An unsubmitted UPDATE plan died
        // with its intent above.)
        release_plan_modal(host_id);
        offer.clear();
    }
    // ADD acceptance is authoritative only until the first applicable GET;
    // updates carry the same gap in their tracked run instead.
    accepted_run_waiting.set(false);
    // Ownership outlives the intent: while a retained accepted/observed run
    // is still nonterminal, the row stays busy even when this read shows an
    // unrelated older Completed. Only the tracked run's own terminal state,
    // vanishment, retarget, or removal releases it.
    let busy = running || retained_run_live(tracked.peek().as_ref());
    on_running.call(busy);
    progress.set(Some(view));
    progress_error.set(None);
}

/// Follow, supersede, or settle the tracked run against one applicable view.
///
/// Runs only when no unsubmitted intent competes (the caller invalidates
/// those first). Observations arrive in order and are applied in order;
/// run ids are compared for identity, never sorted for age.
#[allow(clippy::too_many_arguments)]
fn reconcile_tracked(
    view: &ProvisioningView,
    host_id: HostId,
    binding: &HostBinding,
    epoch_now: u64,
    retired: &HashSet<String>,
    tracked: &mut Signal<Option<TrackedRun>>,
    update_error: &mut Signal<Option<UpdateDiagnostic>>,
    update_warning: &mut Signal<Option<UpdateDiagnostic>>,
) {
    let running = view.run_id.is_some() && view.status == ProvisioningStatus::Running;
    let retired_view = view
        .run_id
        .as_deref()
        .is_some_and(|run_id| retired.contains(run_id));
    let Some(old) = tracked.peek().clone() else {
        // Nothing followed: adopt running or failed UPDATE work (another
        // client's run, or retained failure visibility), and nothing else —
        // and never a retired id, which is old-target evidence.
        let adopt = !retired_view
            && (running && view.operation == Some(ProvisioningOperation::Update)
                || !running
                    && view.status == ProvisioningStatus::Failed
                    && view.operation == Some(ProvisioningOperation::Update)
                    && view.run_id.is_some());
        if adopt {
            tracked.set(Some(TrackedRun {
                host: host_id,
                run_id: view.run_id.clone().unwrap_or_default(),
                operation: view.operation,
                source: RunSource::Observed,
                binding: binding.clone(),
                last_status: Some(view.status),
                started_at: Instant::now(),
            }));
        }
        return;
    };
    if view.run_id.as_deref() == Some(old.run_id.as_str()) {
        // The followed run, reporting in. Only its own authoritative
        // success settles it; anything else updates what was last seen.
        if authoritative_update_success(view, &old) {
            settle_tracked_success(&old, tracked, update_error, update_warning);
        } else {
            tracked.set(Some(TrackedRun {
                last_status: Some(view.status),
                ..old
            }));
        }
        return;
    }
    // A different view than the followed run. The backend excludes overlap,
    // so a tracked run whose terminal state was never observed ended unseen:
    // retain that uncertainty rather than resolving it from this view.
    if old.operation == Some(ProvisioningOperation::Update)
        && !matches!(
            old.last_status,
            Some(ProvisioningStatus::Completed | ProvisioningStatus::Failed)
        )
    {
        update_warning.set(Some(UpdateDiagnostic {
            epoch: epoch_now,
            sticky: true,
            host_changed: false,
            text: format!(
                "update run {} ended without reporting its outcome; following the latest state instead",
                old.run_id
            ),
        }));
    }
    // Follow newer running work of either operation, and failed UPDATE
    // work for its visibility. Completed history, idle, and retired
    // old-target evidence adopt nothing.
    let adopt = !retired_view
        && view.run_id.is_some()
        && (running
            || view.status == ProvisioningStatus::Failed
                && view.operation == Some(ProvisioningOperation::Update));
    if adopt {
        tracked.set(Some(TrackedRun {
            host: host_id,
            run_id: view.run_id.clone().unwrap_or_default(),
            operation: view.operation,
            source: RunSource::Observed,
            binding: binding.clone(),
            last_status: Some(view.status),
            started_at: Instant::now(),
        }));
    } else if view.run_id.is_none()
        && old
            .last_status
            .is_none_or(|status| status == ProvisioningStatus::Running)
    {
        // The followed run vanished into idle — a helm restart dropped its
        // progress — with its outcome never observed. The sticky diagnostic
        // above already holds the row open; release the tracked pair and
        // its busy paint rather than following a run that no longer exists.
        tracked.set(None);
    }
}

/// Settle the tracked run on its own authoritative success.
///
/// Clears the tracked pair and every update diagnostic the success
/// supersedes: for a client-submitted run, everything at or below its
/// submitting epoch. An observed run's success settles only its own
/// disclosure — diagnostics stay for a correlated success or a new attempt.
fn settle_tracked_success(
    old: &TrackedRun,
    tracked: &mut Signal<Option<TrackedRun>>,
    update_error: &mut Signal<Option<UpdateDiagnostic>>,
    update_warning: &mut Signal<Option<UpdateDiagnostic>>,
) {
    tracked.set(None);
    if let RunSource::Submitted { epoch } = old.source {
        if update_error
            .peek()
            .as_ref()
            .is_some_and(|held| held.epoch <= epoch)
        {
            update_error.set(None);
        }
        if update_warning
            .peek()
            .as_ref()
            .is_some_and(|held| held.epoch <= epoch)
        {
            update_warning.set(None);
        }
    }
}

/// Record a planning outcome that submitted nothing.
///
/// Planning failures, manual replies, and unvalidated plans all describe the
/// attempt they came from — unless a sticky uncertainty from an earlier
/// submission is still held. That earlier request may have committed while
/// this attempt provably submitted nothing, so the uncertainty outranks the
/// new outcome: the planning diagnostic is dropped, and only a correlated
/// success or removal clears what stands.
fn set_planning_diagnostic(
    update_error: &mut Signal<Option<UpdateDiagnostic>>,
    epoch: u64,
    text: String,
) {
    if update_error.peek().as_ref().is_some_and(|held| held.sticky) {
        return;
    }
    update_error.set(Some(UpdateDiagnostic {
        epoch,
        sticky: false,
        host_changed: false,
        text,
    }));
}

/// The panel's actual in-flow contribution. A body with no children still
/// defeats `.provisioning-panel:empty`, so it must remain distinct from an
/// empty panel even when its only offer is a fixed-position modal.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ProvisioningFlow {
    Empty,
    Trace(ProvisioningTraceShape),
    Body(Box<ProvisioningBody>),
}

/// Only content drawn below the host header participates here. Modal
/// payloads, control callbacks, copy bytes and lifecycle identity stay with
/// their existing owners; the defensive inline Update plan is real flow.
#[derive(Clone, Debug)]
struct ProvisioningBody {
    run: Option<ExpandedRun>,
    inline_plan: Option<PlanBody>,
    pending: Option<PendingLine>,
    error: Option<FlowParts>,
    warning: Option<FlowParts>,
    read_error: Option<FlowParts>,
    manual: Option<ManualLine>,
}

impl PartialEq for ProvisioningBody {
    fn eq(&self, other: &Self) -> bool {
        let same_plan = match (&self.inline_plan, &other.inline_plan) {
            (Some(left), Some(right)) => left.same_flow(right),
            (None, None) => true,
            _ => false,
        };
        self.run == other.run
            && same_plan
            && self.pending == other.pending
            && self.error == other.error
            && self.warning == other.warning
            && self.read_error == other.read_error
            && self.manual == other.manual
    }
}

impl Eq for ProvisioningBody {}

/// A run's displayed controls and prose can rewrap independently of its
/// identity. The fold id remains available for the existing guarded handler,
/// but equality compares only whether that control occupies the header.
#[derive(Clone, Debug)]
struct ExpandedRun {
    operation: &'static str,
    status: &'static str,
    fold_run: Option<String>,
    steps: Vec<ExpandedStep>,
    message: Option<FlowParts>,
}

impl PartialEq for ExpandedRun {
    fn eq(&self, other: &Self) -> bool {
        self.operation == other.operation
            && self.status == other.status
            && self.fold_run.is_some() == other.fold_run.is_some()
            && self.steps == other.steps
            && self.message == other.message
    }
}

impl Eq for ExpandedRun {}

/// Keep the original step attributes for CSS and browser handles. They do
/// not add flow: the marker has fixed geometry, while the displayed status
/// word owns an auto-sized column and can change its neighbor's wrapping.
#[derive(Clone, Debug)]
struct ExpandedStep {
    id: String,
    raw_status: String,
    name: String,
    status: String,
    message: Option<FlowParts>,
}

impl PartialEq for ExpandedStep {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.status == other.status && self.message == other.message
    }
}

impl Eq for ExpandedStep {}

/// Offer precedence is resolved before choosing this line. The variants
/// carry both its existing class and words, so rendering cannot drift from
/// the value used to detect a content change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingLine {
    CheckingLocalSetup,
    Planning,
    WaitingToSubmit,
    Submitting,
}

impl PendingLine {
    /// Preserve the separate CSS hooks even where two stages have the same
    /// dimensions at today's width.
    fn class(self) -> &'static str {
        match self {
            Self::CheckingLocalSetup | Self::Planning => "provisioning-planning",
            Self::WaitingToSubmit => "provisioning-waiting",
            Self::Submitting => "provisioning-submitting",
        }
    }

    /// These are the existing authored stage labels; no lifecycle model is
    /// exposed through the render value.
    fn text(self) -> &'static str {
        match self {
            Self::CheckingLocalSetup => "checking automatic setup…",
            Self::Planning => "planning…",
            Self::WaitingToSubmit => "waiting to submit…",
            Self::Submitting => "submitting…",
        }
    }
}

/// Secondary manual advice has an extra top margin. Its class therefore
/// belongs in equality alongside the actual peer-isolated remedy.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ManualLine {
    parts: FlowParts,
    secondary: bool,
}

/// Borrow only facts the current renderer has already resolved. This is a
/// pure render boundary, not a second lifecycle snapshot or async getter;
/// the existing signal reads and their short-circuit subscriptions stay at
/// the component's original read sites.
#[derive(Default)]
struct ProvisioningFlowInput<'a> {
    snapshot: Option<&'a ProvisioningView>,
    offer: Option<&'a PendingPlan>,
    details_open: bool,
    local_setup: bool,
    manual_remedy: Option<&'a [DetailPart]>,
    is_planning: bool,
    is_waiting_claim: bool,
    is_submitting: bool,
    accepted_run_waiting: bool,
    local_auto_retry: bool,
    error: Option<&'a str>,
    warning: Option<&'a str>,
    read_error: Option<&'a str>,
    fold_run: Option<&'a str>,
    visible_trace: Option<ProvisioningTraceShape>,
}

impl ProvisioningFlow {
    /// Resolve exactly the branches the panel draws, including an empty
    /// body's wrapper and the in-flow defensive confirmation. Hidden runs,
    /// hover data and ids never become movement causes through this value.
    fn resolve(input: ProvisioningFlowInput<'_>) -> Self {
        let has_details_content = input.snapshot.is_some_and(|view| view.run_id.is_some())
            || input.offer.is_some()
            || input.is_planning
            || input.is_waiting_claim
            || input.is_submitting
            || input.error.is_some()
            || input.warning.is_some()
            || input.read_error.is_some()
            || (input.local_setup && input.manual_remedy.is_some());
        if !((input.details_open && has_details_content) || input.offer.is_some()) {
            return input.visible_trace.map_or(Self::Empty, Self::Trace);
        }
        let line = |text: &str| FlowParts(vec![DetailPart::peer(text)]);
        let run = input
            .details_open
            .then_some(input.snapshot)
            .flatten()
            .filter(|view| view.run_id.is_some())
            .map(|view| ExpandedRun {
                operation: operation_label(view.operation),
                status: status_label(view.status),
                fold_run: input.fold_run.map(str::to_owned),
                steps: view
                    .steps
                    .iter()
                    .map(|step| ExpandedStep {
                        id: step.step.clone(),
                        raw_status: step.status.clone(),
                        name: display_peer(progress_step_name(step)),
                        status: progress_step_status(&step.status).to_owned(),
                        message: step.message.as_ref().map(|text| line(text.raw())),
                    })
                    .collect(),
                message: view.message.as_ref().map(|text| line(text.raw())),
            });
        let inline_plan = input
            .offer
            .filter(|plan| {
                !matches!(
                    plan.operation,
                    ProvisioningOperation::Add | ProvisioningOperation::Uninstall
                )
            })
            .map(|plan| PlanBody::new(&plan.confirmation, plan.steps.as_ref()));
        let pending = if input.offer.is_some() || !input.details_open {
            None
        } else if input.is_planning {
            Some(if input.local_setup {
                PendingLine::CheckingLocalSetup
            } else {
                PendingLine::Planning
            })
        } else if input.is_waiting_claim {
            Some(PendingLine::WaitingToSubmit)
        } else if input.is_submitting {
            Some(PendingLine::Submitting)
        } else {
            None
        };
        let run_active = input.snapshot.is_some_and(|view| {
            view.run_id.is_some() && view.status == ProvisioningStatus::Running
        });
        let failed_operation = input.snapshot.and_then(|view| {
            (view.run_id.is_some() && view.status == ProvisioningStatus::Failed)
                .then_some(view.operation)
                .flatten()
        });
        let automatic_local_remedy = input.local_setup
            && (input.offer.is_some()
                || input.is_planning
                || run_active
                || input.accepted_run_waiting
                || failed_operation.is_some()
                || input.local_auto_retry);
        let manual = (input.details_open && input.local_setup)
            .then_some(input.manual_remedy)
            .flatten()
            .map(|parts| ManualLine {
                parts: FlowParts(parts.to_vec()),
                secondary: automatic_local_remedy,
            });
        Self::Body(Box::new(ProvisioningBody {
            run,
            inline_plan,
            pending,
            error: input
                .details_open
                .then_some(input.error)
                .flatten()
                .map(line),
            warning: input
                .details_open
                .then_some(input.warning)
                .flatten()
                .map(line),
            read_error: input
                .details_open
                .then_some(input.read_error)
                .flatten()
                .map(line),
            manual,
        }))
    }
}

/// Capture only changes to the panel's actual body before its DOM publishes.
/// Folded trace changes retain their existing shape route. The shared hook
/// owns baseline seeding and guarded deferred delivery; this projection adds
/// no lifecycle state and keeps an empty body's real wrapper occupancy.
fn use_provisioning_flow_movement(
    flow: &ProvisioningFlow,
    host_id: HostId,
    capture_lower_hosts: Callback<HostId, MovementTicket>,
) {
    let body = match flow {
        ProvisioningFlow::Body(body) => Some(body.clone()),
        ProvisioningFlow::Empty | ProvisioningFlow::Trace(_) => None,
    };
    use_rendered_movement(&body, || capture_lower_hosts.call(host_id));
}

/// One registered host's provisioning actions and latest retained run.
///
/// `manual_remedy` is passed for every phase because the host row derives it
/// once. This component consumes and renders it only for the reserved local
/// setup state, where the automatic offer replaces it as the primary
/// affordance without removing the manual escape hatch.
#[component]
pub(crate) fn ProvisioningPanel(
    host: Host,
    mut ops: OpLock,
    /// Whether this row shows full details: the global checkbox OR this
    /// row's automatic disclosure, computed by the parent. The checkbox is
    /// the user's preference and this panel never writes it for an update;
    /// the automatic half is published into `auto_details` below.
    details_open: bool,
    /// The global preference independently holds rows open. A one-off fold
    /// control only applies when this preference is off.
    global_details_open: bool,
    /// Whether this row's manual local remedy should be replaced by an
    /// automatic ADD probe and offer.
    local_setup: bool,
    /// The exact manual fallback derived from the host phase.
    manual_remedy: Option<Vec<DetailPart>>,
    /// One-shot menu requests keyed by host.
    mut action_requests: Signal<HashMap<HostId, ActionRequest>>,
    /// Each mounted row's current contribution to the host actions menu.
    mut menu_states: Signal<HashMap<HostId, ProvisioningMenuState>>,
    /// Collapsed traces whose presence changes fixed-surface geometry.
    mut trace_shapes: Signal<HashMap<HostId, ProvisioningTraceShape>>,
    /// Rows whose automatic disclosure currently holds them open. A confirmed
    /// setup or uninstall owns one expansion per run; failures and unresolved
    /// diagnostics also open their row. The parent ORs this with the global
    /// checkbox per row.
    mut auto_details: Signal<HashSet<HostId>>,
    /// Compact row status published beside the automatic-disclosure set.
    mut row_update_progress: Signal<HashMap<HostId, HostUpdateProgress>>,
    /// Reveal global details for the local ADD handoff's hidden diagnostic.
    /// Remote confirmations stay visible even when every row is folded.
    on_reveal_details: EventHandler<()>,
    /// Capture current openings strictly below this host in the retained
    /// displayed vector. The parent owns row selection and host admission;
    /// this panel contributes only its actual in-flow content change.
    capture_lower_hosts: Callback<HostId, MovementTicket>,
    /// Paint-only busy bookkeeping for this row in the parent. It never
    /// excludes a run.
    on_running: EventHandler<bool>,
    /// Ask the authoritative registry to refresh after registration or an
    /// accepted run.
    on_changed: EventHandler<()>,
    /// Notices owed for confirmed uninstalls, keyed by host. This panel adds
    /// its host's entry as it submits and drops it on a refusal or a failed
    /// run; the parent shows it once the host leaves the list, since this
    /// panel unmounts with its row and the helm keeps no result for a host
    /// it forgot.
    mut uninstall_notices: Signal<HashMap<HostId, UninstallNotice>>,
    /// Panel-owned refusals outlive row detail disclosure and show one modal
    /// at a time across hosts.
    mut refusals: Signal<HashMap<HostId, HostRefusalEntry>>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut preferences = use_context::<SharedPreferences>();
    let host_id = host.id;
    let binding = HostBinding::from(&host);
    let current_binding = use_memo(use_reactive((&binding,), |(binding,)| binding));
    let progress = use_signal(|| None::<ProvisioningView>);
    let progress_error = use_signal(|| None::<String>);
    let progress_surface = use_signal(SurfaceReader::default);
    let mut read_gate = use_signal(ReadGate::default);
    // The plan this row is asking the user to confirm (Setup, Uninstall, or
    // the defensive inline Update), keyed by the plan itself. Its answers
    // carry the token of the showing their render drew, so they act on that
    // plan and its `probe_id` only, and only while that showing is still
    // open. Opening a new plan, the binding and run reconciliation below,
    // and a new menu request retire it with `clear`, never by answering it.
    let mut offer = use_confirm_slot::<PendingPlan, ()>();
    // The showing a remembered "don't ask in the future" answer is for: the
    // token `offer.open` returned when planning opened a remote ADD plan
    // with that preference set. Holding the token, not a bare flag, is what
    // keeps the remembered answer bound to that one plan. If reconciliation
    // clears the plan before the auto-submit effect runs, the token goes
    // stale and answers nothing, so no later plan (one the user was never
    // meant to skip) can be confirmed by it.
    let mut auto_submit_add = use_signal(|| None::<ConfirmToken<PendingPlan>>);
    // A confirmed action owns this slot from click through its plan reply,
    // letting the folded row name the pending operation.
    let mut planning = use_signal(|| None::<ProvisioningOperation>);
    let mut action_error = use_signal(|| None::<String>);
    let mut action_warning = use_signal(|| None::<String>);
    let mut update_error = use_signal(|| None::<UpdateDiagnostic>);
    let mut update_warning = use_signal(|| None::<UpdateDiagnostic>);
    let mut intent = use_signal(|| None::<UpdateIntent>);
    let mut intent_epoch = use_signal(|| 0_u64);
    // The update POST currently in flight, if any. Set at claim, cleared
    // when that POST's outcome installs; the read path suppresses progress
    // commits while this stands. Separate from the intent because the
    // binding watcher can drop the intent on a retarget while the POST is
    // still out — suppression must survive that, or a read would adopt the
    // old target's run as the new target's work.
    let mut outstanding_submission = use_signal(|| None::<OutstandingSubmission>);
    let mut tracked = use_signal(|| None::<TrackedRun>);
    // Confirmed setup and uninstall are not tracked like automatic Update.
    // Their first observed run snapshot starts the row's elapsed clock.
    let mut confirmed_run_clock = use_signal(|| None::<(String, Instant)>);
    let mut confirmed_disclosure = use_signal(|| None::<ConfirmedRunDisclosure>);
    // Run ids a target change detached from this row. The progress route is
    // host-scoped, not target-scoped, so the helm can keep reporting the old
    // target's retained run after a retarget; adopting it would reinterpret
    // old evidence as the new target's history. Retired ids are never
    // adopted again (fresh runs mint fresh UUIDs), and die with the row.
    let mut retired_runs = use_signal(HashSet::<String>::new);
    // Acceptance is authoritative before the first follow-up GET. Retain
    // that short-lived fact so the local manual command cannot become the
    // primary remedy while automatic setup is already running. ADD only;
    // updates carry the same gap in their tracked run.
    let accepted_run_waiting = use_signal(|| false);
    // Once automatic local setup proved possible (or failed before it could
    // decide), cancel and retry return to an explicit offer. A Manual reply
    // turns this off because repeating an unsupported probe is not a remedy.
    let mut local_auto_retry = use_signal(|| false);

    // The parent owns the provisioning-busy set. Removing this row must
    // remove only this component's contribution to it; mutation-busy lives
    // in a different set and is unaffected. Intent, plan, tracked run, and
    // diagnostics die with the component's own signals; the parent-map
    // entries need explicit removal or a late response could not recreate
    // them but the menu geometry still would count them.
    use_drop(move || {
        on_running.call(false);
        action_requests.write().remove(&host_id);
        menu_states.write().remove(&host_id);
        trace_shapes.write().remove(&host_id);
        auto_details.write().remove(&host_id);
        row_update_progress.write().remove(&host_id);
    });

    // One reader per row. A feed bump says only that something changed, so
    // every mounted row re-reads its own host-scoped view; the reader
    // coalesces bursts and keeps retry demand alive after a failed request.
    // Every dispatch carries a read-gate generation: the gate is fenced at
    // intent acceptance and at submission, so a delayed pre-submission GET
    // can settle nothing — neither the disclosure, nor the tracked run, nor
    // the diagnostics. What a fenced read describes belongs to a lifecycle
    // that has already moved on. And a GET that completes while an update
    // POST is still outstanding commits nothing at all (it reports
    // unanswered, keeping retry demand alive): the submission outcome
    // installs first, then its explicit reread fetches the post-submission
    // world fresh. That suppression keys on the outstanding POST, not the
    // planning intent, because a retarget drops the intent mid-POST.
    let read_base = base.clone();
    let read_progress = move || {
        let generation = read_gate.write().start();
        let base = read_base.clone();
        let mut gate = read_gate;
        let mut view_progress = progress;
        let mut view_error = progress_error;
        let view_running = on_running;
        let mut view_offer = offer;
        let mut view_waiting = accepted_run_waiting;
        let mut view_intent = intent;
        let view_outstanding = outstanding_submission;
        let mut view_epoch = intent_epoch;
        let mut view_tracked = tracked;
        let mut view_update_error = update_error;
        let mut view_update_warning = update_warning;
        let retired_now = retired_runs.peek().clone();
        let view_binding = current_binding;
        async move {
            match fetch_provisioning(&base, host_id).await {
                Ok(view) => {
                    if !gate.write().accept_success(generation) {
                        return true;
                    }
                    // A submission is still outstanding: this read raced the
                    // pending POST, and whatever it describes — another
                    // client's newer run, or the old target's run after a
                    // retarget — the delayed outcome would overwrite when it
                    // installs. Commit nothing: not the view, not the tracked
                    // run, not the busy paint. Report unanswered so the reader
                    // keeps retry demand alive; the outcome installs first and
                    // its explicit reread fetches the post-submission world
                    // fresh. Keyed on the outstanding POST rather than the
                    // intent's phase: the watcher drops the intent on a
                    // retarget while the POST is still out, and suppression
                    // must survive that.
                    if view_outstanding.peek().is_some() {
                        return false;
                    }
                    let epoch_now = *view_epoch.peek();
                    let binding_now = view_binding.peek().clone();
                    reconcile_progress(
                        view,
                        ProgressCommit {
                            host_id,
                            binding: binding_now,
                            epoch_now,
                            retired: retired_now,
                            intent: &mut view_intent,
                            epoch: &mut view_epoch,
                            tracked: &mut view_tracked,
                            offer: &mut view_offer,
                            accepted_run_waiting: &mut view_waiting,
                            update_error: &mut view_update_error,
                            update_warning: &mut view_update_warning,
                            progress: &mut view_progress,
                            progress_error: &mut view_error,
                            on_running: view_running,
                        },
                    );
                    true
                }
                Err(error) => {
                    if !gate.peek().accept_failure(generation) {
                        return true;
                    }
                    view_error.set(Some(error));
                    false
                }
            }
        }
    };
    let request_progress =
        move |trigger: Trigger| request_read(progress_surface, trigger, read_progress.clone());

    let mount_progress = request_progress.clone();
    use_hook(move || mount_progress(Trigger::Explicit));
    let feed_progress = request_progress.clone();
    use_feed_reader(move || feed_progress(Trigger::Notice));
    let fallback_progress = request_progress.clone();
    use_future(move || {
        let request = fallback_progress.clone();
        async move {
            loop {
                fallback_sleep().await;
                if fallback_polls_now() {
                    request(Trigger::Scheduled);
                }
            }
        }
    });

    // ADD preparation is discovery-first and may register an answering
    // supervisor, but it never starts a provisioning run or holds OpLock
    // across transport inspection. It still refuses behind the page lock;
    // the consumption effect below retries it when that clears.
    let plan_base = base.clone();
    let plan_host = host.clone();
    let plan_progress = request_progress.clone();
    // UNINSTALL plans through the same path: it, too, shows the helm's plan
    // and submits only on an explicit confirmation. Only ADD carries the
    // local-setup retry and the remembered "don't ask again" answer.
    let begin_confirmed_plan = move |operation: ProvisioningOperation| -> bool {
        if planning.peek().is_some() || ops.busy_now() {
            return false;
        }
        planning.set(Some(operation));
        release_plan_modal(host_id);
        offer.clear();
        action_error.set(None);
        action_warning.set(None);
        let base = plan_base.clone();
        let host = plan_host.clone();
        let requested_binding = HostBinding::from(&host);
        let is_local_add = host.kind.sets_up_locally() && operation == ProvisioningOperation::Add;
        let remembered_answer = operation == ProvisioningOperation::Add
            && preferences.0.peek().skip_host_setup_confirmation == Some(true);
        let reread = plan_progress.clone();
        spawn(async move {
            let prepared = prepare(&base, &host, operation).await;
            if *current_binding.peek() != requested_binding {
                planning.set(None);
                return;
            }
            match prepared {
                Ok(Preparation::Plan(plan)) => {
                    if is_local_add {
                        local_auto_retry.set(true);
                    }
                    let opened = offer.open(*plan, ());
                    auto_submit_add.set((!is_local_add && remembered_answer).then_some(opened));
                }
                Ok(Preparation::Discovered) => {
                    // Discovery can register a host and can resolve a
                    // retained failed ADD run, so both authorities move.
                    if is_local_add {
                        local_auto_retry.set(false);
                    }
                    reread(Trigger::Explicit);
                    on_changed.call(());
                }
                Ok(Preparation::Manual(reason)) => {
                    if is_local_add {
                        local_auto_retry.set(false);
                    }
                    if is_local_add {
                        action_error.set(Some(reason));
                    } else {
                        refusals.write().insert(
                            host_id,
                            HostRefusalEntry::new(
                                &host,
                                planning_refusal(operation, reason.into()),
                            ),
                        );
                    }
                    if is_local_add {
                        on_reveal_details.call(());
                    }
                }
                Ok(Preparation::Unvalidated(problem)) => {
                    if is_local_add {
                        local_auto_retry.set(true);
                    }
                    action_error.set(Some(problem));
                    on_changed.call(());
                    if is_local_add {
                        on_reveal_details.call(());
                    }
                }
                Err(error) => {
                    if is_local_add {
                        local_auto_retry.set(true);
                    }
                    if is_local_add {
                        action_error.set(Some(error.reason));
                    } else {
                        refusals.write().insert(
                            host_id,
                            HostRefusalEntry::new(&host, planning_refusal(operation, error)),
                        );
                    }
                    if is_local_add {
                        on_reveal_details.call(());
                    }
                }
            }
            planning.set(None);
        });
        true
    };

    // Accept one automatic-update click: capture its binding and a fresh
    // intent epoch synchronously, publish the row's `updating…` status
    // through the intent (the row stays folded), and enter Planning.
    // Planning mutates nothing, so this never consults the page lock; the
    // submission claim retries reactively instead.
    let update_base = base.clone();
    let update_host = host.clone();
    let update_progress = request_progress.clone();
    let begin_update = move || {
        let epoch_now = *intent_epoch.peek() + 1;
        intent_epoch.set(epoch_now);
        read_gate.write().fence();
        // A new attempt supersedes a merely displayed ADD offer and the
        // previous attempt's own diagnostics. Sticky submission
        // uncertainty survives: only this attempt's correlated success
        // supersedes that.
        release_plan_modal(host_id);
        offer.clear();
        // Setup/uninstall row diagnostics can still occupy the Update display
        // slots. A new Update must not hide its own result behind one.
        action_error.set(None);
        action_warning.set(None);
        if update_error
            .peek()
            .as_ref()
            .is_some_and(|held| !held.sticky)
        {
            update_error.set(None);
        }
        if update_warning
            .peek()
            .as_ref()
            .is_some_and(|held| !held.sticky)
        {
            update_warning.set(None);
        }
        let requested = current_binding.peek().clone();
        intent.set(Some(UpdateIntent {
            epoch: epoch_now,
            binding: requested.clone(),
            phase: IntentPhase::Planning,
            plan: None,
        }));
        let base = update_base.clone();
        let host = update_host.clone();
        let reread = update_progress.clone();
        spawn(async move {
            let prepared = prepare(&base, &host, ProvisioningOperation::Update).await;
            // A newer click, a retarget, or a foreign run orphans this
            // future through the epoch; a stale completion restores
            // nothing, and in particular submits nothing.
            let live = intent.peek().as_ref().is_some_and(|live| {
                live.epoch == epoch_now
                    && live.phase == IntentPhase::Planning
                    && live.binding == requested
            }) && *current_binding.peek() == requested;
            if !live {
                return;
            }
            match prepared {
                Ok(Preparation::Plan(plan)) => {
                    if automatic_update(plan.operation, &plan.binding) {
                        // Store the one-use plan, enter WaitingForClaim,
                        // and let the claim effect attempt submission.
                        intent.set(Some(UpdateIntent {
                            epoch: epoch_now,
                            binding: requested,
                            phase: IntentPhase::WaitingForClaim,
                            plan: Some(*plan),
                        }));
                    } else {
                        // Defensive: only remote updates skip
                        // confirmation. The backend refuses local update
                        // plans, so this arm should not happen — but if
                        // it ever does, the plan confirms explicitly
                        // rather than submitting itself.
                        intent.set(None);
                        offer.open(*plan, ());
                        on_reveal_details.call(());
                    }
                }
                Ok(Preparation::Discovered) => {
                    // Unreachable for updates (`prepare` maps every
                    // successful update plan to `Plan`), but defensive:
                    // end the intent and reread rather than stranding it.
                    intent.set(None);
                    reread(Trigger::Explicit);
                    on_changed.call(());
                }
                Ok(Preparation::Manual(reason)) => {
                    intent.set(None);
                    refusals.write().insert(
                        host_id,
                        HostRefusalEntry::new(
                            &host,
                            planning_refusal(ProvisioningOperation::Update, reason.into()),
                        ),
                    );
                }
                Ok(Preparation::Unvalidated(problem)) => {
                    intent.set(None);
                    set_planning_diagnostic(&mut update_error, epoch_now, problem);
                    on_changed.call(());
                }
                Err(error) => {
                    // Planning failure submits nothing. Its modal is separate
                    // from any sticky uncertainty an earlier submission left
                    // in the row.
                    intent.set(None);
                    refusals.write().insert(
                        host_id,
                        HostRefusalEntry::new(
                            &host,
                            planning_refusal(ProvisioningOperation::Update, error),
                        ),
                    );
                }
            }
        });
    };

    // The row menu writes a one-shot request while this component keeps the
    // async lifecycle. Ownership is synchronous at acceptance: the decision
    // reads live lifecycle state — the intent and the retained tracked run —
    // so a second same-task click coalesces into the accepted ownership
    // instead of becoming a second plan that starts when the first one's
    // planning flag clears.
    let mut consume_request = {
        let mut begin_confirmed = begin_confirmed_plan.clone();
        let mut begin_auto = begin_update;
        move |request: &ActionRequest| -> bool {
            // Snapshot the decision inputs as owned values first. A peek
            // guard borrowed into the match below would stay alive across
            // the planning sets and spawns in its arms, and that borrow
            // panics the wasm runtime (observed directly: two
            // `RuntimeError: unreachable` per click, zero requests sent).
            let current = current_binding.peek().clone();
            let owned = update_owned(intent.peek().as_ref(), tracked.peek().as_ref());
            let add_planning = planning.peek().is_some();
            let busy = ops.busy_now();
            match decide_request(request, &current, owned, add_planning, busy) {
                RequestDecision::Refuse => false,
                RequestDecision::Stale | RequestDecision::Coalesce => true,
                RequestDecision::Plan => match request.operation {
                    ProvisioningOperation::Update => {
                        begin_auto();
                        true
                    }
                    operation @ (ProvisioningOperation::Add | ProvisioningOperation::Uninstall) => {
                        begin_confirmed(operation)
                    }
                },
            }
        }
    };
    let update_submission_host = host.clone();
    use_effect(move || {
        planning();
        ops.busy();
        intent();
        tracked();
        let accepted = accepted_request(&action_requests.read(), host_id, &mut consume_request);
        if accepted.is_some_and(|request| action_requests.peek().get(&host_id) == Some(&request)) {
            action_requests.write().remove(&host_id);
        }
    });

    // The submission-claim retry for automatic updates. It subscribes to
    // the pending intent and the page lock, and to nothing else: no click,
    // no render beat, no feed notice, no poll. Planning completion only
    // stores the plan and enters WaitingForClaim; this effect is both the
    // immediate attempt and every later retry, and a failed claim keeps
    // the exact plan and epoch while sending nothing. The POST itself is
    // spawned once, at claim, and never retried.
    let claim_base = base.clone();
    let claim_progress = request_progress.clone();
    use_effect(move || {
        ops.busy();
        let Some(live) = intent().clone() else {
            return;
        };
        if live.phase != IntentPhase::WaitingForClaim {
            return;
        }
        // Recheck everything the wait may have invalidated before touching
        // the token: the row, the host kind, and competing runs.
        if live.binding != *current_binding.peek() || !live.binding.kind.updates_automatically() {
            intent.set(None);
            return;
        }
        if let Some(running_view) = progress
            .peek()
            .clone()
            .filter(|view| view.run_id.is_some() && view.status == ProvisioningStatus::Running)
        {
            // A run landed without invalidating the intent — a read that
            // raced the wait. Invalidate here and follow it; never claim
            // behind a live run.
            let retired_now = retired_runs.peek().clone();
            invalidate_intent_for_run(
                &running_view,
                host_id,
                &current_binding.peek().clone(),
                &retired_now,
                &mut intent,
                &mut intent_epoch,
                &mut tracked,
                &mut update_warning,
            );
            return;
        }
        let Some(plan) = live.plan.clone() else {
            intent.set(None);
            return;
        };
        // Peek before claiming. `claim_guard` takes a write borrow, and
        // dropping that borrow notifies `held` subscribers even when the
        // claim changes nothing — including this effect, which subscribes
        // through `ops.busy()`. Claiming blindly while another operation
        // owns the token would therefore re-trigger this effect on every
        // failed attempt: an infinite reactive loop that wedges the page
        // (observed: renderer pegged, every page-channel command hanging,
        // the waiting indicator never becoming observable). The peek keeps
        // the effect purely reactive: it reruns only when `held` or the
        // intent actually changes, and the release below is what wakes it.
        // Peek-then-claim is atomic here — the effect body runs
        // synchronously with no await between the two — and the `else`
        // below stays as the belt-and-braces refusal.
        if ops.busy_now() {
            return;
        }
        let mut claim_ops = ops;
        let Some(claim) = claim_ops.claim_guard() else {
            return;
        };
        // Claimed: remove the plan, enter Submitting, record the
        // outstanding POST independently of the intent, then spawn it. The
        // fence rejects reads that overlapped the wait.
        read_gate.write().fence();
        let epoch = live.epoch;
        let submitting = live.binding.clone();
        intent.set(Some(UpdateIntent {
            phase: IntentPhase::Submitting,
            plan: None,
            ..live
        }));
        outstanding_submission.set(Some(OutstandingSubmission {
            epoch,
            binding: submitting,
        }));
        submit_plan(
            Submission {
                base: claim_base.clone(),
                host_id,
                host_snapshot: update_submission_host.clone(),
                submitted_epoch: Some(epoch),
                claim,
                current_binding,
                intent,
                outstanding: outstanding_submission,
                tracked,
                retired_runs,
                read_gate,
                action_error,
                action_warning,
                update_error,
                update_warning,
                accepted_run_waiting,
                local_auto_retry,
                on_running,
                on_changed,
                uninstall_notices,
                refusals,
                reread: claim_progress.clone(),
            },
            plan,
        );
    });

    // Unsubmitted update work belongs to its exact row; a submitted run
    // belongs to its target. A binding change fences in-flight reads (they
    // describe the old row) and immediately rereads, because a fenced read
    // answers nothing and the notice that triggered it is already consumed:
    // without the fresh read the row would sit on stale progress until some
    // unrelated later notice. It also drops the intent and its plan unsent
    // with an epoch bump so stale planning cannot restore either. That
    // includes a Submitting intent, whose POST is still outstanding: the
    // outcome still installs when it lands, and progress suppression (keyed
    // on the outstanding submission, not this intent) holds until it does.
    // On a real target change the watcher also detaches the followed run
    // with an explicit unresolved diagnostic instead of silently collapsing.
    // Incarnation churn alone breaks nothing: the update changing its own
    // connection is normal. Detached run ids retire so the host-scoped
    // progress route cannot reintroduce old-target evidence as the new
    // target's history. Learning the first identity can be the update's own
    // reconnect, but pin it immediately: otherwise a later replacement
    // would keep comparing against the original unknown identity.
    let watcher_progress = request_progress.clone();
    use_effect(use_reactive((&binding,), move |(binding,)| {
        // The intent can already be gone while its POST waits. Preserve the
        // first identity in the longer-lived submission too, so None→A→B
        // before acceptance cannot be mistaken for a single first contact.
        let submission = outstanding_submission.peek().clone();
        if let Some(mut submission) = submission
            && submission.binding.identity.is_none()
            && binding.identity.is_some()
            && same_update_target(&submission.binding, &binding)
        {
            submission.binding.identity = binding.identity.clone();
            outstanding_submission.set(Some(submission));
        }
        let followed = tracked.peek().clone();
        let target_changed = followed
            .as_ref()
            .is_some_and(|followed| !same_update_target(&followed.binding, &binding));
        let intent_stale = intent
            .peek()
            .as_ref()
            .is_some_and(|live| live.binding != binding);
        if intent_stale || target_changed {
            read_gate.write().fence();
            watcher_progress(Trigger::Explicit);
        }
        if intent_stale {
            let counter = *intent_epoch.peek() + 1;
            intent_epoch.set(counter);
            intent.set(None);
        }
        if let Some(followed) = followed.as_ref().filter(|_| target_changed) {
            retired_runs.write().insert(followed.run_id.clone());
            tracked.set(None);
            on_running.call(false);
            let warning = changed_host_diagnostic(
                update_warning.peek().as_ref(),
                *intent_epoch.peek(),
                &followed.binding,
                &binding,
                false,
            );
            if let Some(warning) = warning {
                update_warning.set(Some(warning));
            }
        } else if let Some(mut followed) = followed
            && followed.binding.identity.is_none()
            && binding.identity.is_some()
        {
            followed.binding.identity = binding.identity.clone();
            tracked.set(Some(followed));
        }
    }));

    // Subscribe to the warning as well as host/build facts: a held acceptance
    // can install it after the host is already current. An owned snapshot
    // releases the signal read before clearing, avoiding a read/write borrow
    // conflict. This clears advice only; retired runs and other diagnostics
    // keep their existing lifetimes and can never be settled by Connected.
    use_effect(use_reactive((&host.state,), move |(state,)| {
        let warning = update_warning();
        let skew = crate::skew::HELM_BUILD_SKEW.read().clone();
        let seen = *crate::skew::HELM_BUILD_SEEN.read();
        if warning.as_ref().is_some_and(|warning| {
            change_warning_resolved(warning, &state, known_helm_build(skew.as_ref(), seen))
        }) {
            update_warning.set(None);
        }
    }));

    // Publish the row's disclosure and compact status together. Update stays
    // folded while running; confirmed actions expand once per run id. Errors,
    // warnings, and retained failed runs keep their diagnosis visible.
    use_effect(move || {
        let current_intent = intent();
        let current_tracked = tracked();
        let current_progress = progress();
        let error = update_error();
        let warning = update_warning();
        let read_error = progress_error();
        let action_diagnostic = action_error().is_some() || action_warning().is_some();
        let prior_confirmed = confirmed_disclosure();
        let next_confirmed =
            confirmed_run_disclosure(prior_confirmed.as_ref(), current_progress.as_ref());
        if prior_confirmed != next_confirmed {
            confirmed_disclosure.set(next_confirmed.clone());
        }
        let disclosed = action_diagnostic
            || next_confirmed
                .as_ref()
                .is_some_and(|run| !run.folded || run.failed)
            || update_auto_disclosed(
                current_tracked.as_ref(),
                error.as_ref(),
                warning.as_ref(),
                read_error.as_deref(),
            );
        let held = auto_details.peek().contains(&host_id);
        if disclosed && !held {
            auto_details.write().insert(host_id);
        } else if !disclosed && held {
            auto_details.write().remove(&host_id);
        }

        // Start the elapsed clock on the first running snapshot of a
        // confirmed action, keyed by run id so a later run starts fresh.
        let running_confirmed = current_progress
            .as_ref()
            .filter(|view| {
                matches!(
                    view.operation,
                    Some(ProvisioningOperation::Add | ProvisioningOperation::Uninstall)
                ) && view.status == ProvisioningStatus::Running
            })
            .and_then(|view| view.run_id.clone());
        let previous_clock = confirmed_run_clock.peek().clone();
        let clock = match (running_confirmed, previous_clock) {
            (Some(run), Some((seen, at))) if seen == run => Some((seen, at)),
            (Some(run), _) => {
                let started = (run, Instant::now());
                confirmed_run_clock.set(Some(started.clone()));
                Some(started)
            }
            (None, previous) => {
                if previous.is_some() {
                    confirmed_run_clock.set(None);
                }
                None
            }
        };
        let row_progress = planning()
            .map(HostUpdateProgress::PendingConfirmed)
            .or_else(|| {
                derive_row_update_progress(
                    current_intent.as_ref(),
                    current_tracked.as_ref(),
                    current_progress.as_ref(),
                    clock.as_ref(),
                )
            });
        let previous = row_update_progress.peek().get(&host_id).cloned();
        if previous != row_progress {
            if let Some(row_progress) = row_progress {
                row_update_progress.write().insert(host_id, row_progress);
            } else {
                row_update_progress.write().remove(&host_id);
            }
        }
    });

    // A local down-state probes once after the progress authority has said
    // IDLE. Waiting for that first view prevents a reload during a live run
    // from displaying a second setup plan beside it. Both the prop and the
    // row binding are explicit dependencies; tracked signal reads cover the
    // idle/running transition and the page token becoming available.
    let mut local_probe_started = use_signal(|| false);
    // Setup-specific diagnostics belong to a state transition, not to the
    // lifetime of the row. Keep the previous prop explicitly so leaving the
    // state clears them even when the path was a failed-run rerun rather
    // than the initial automatic probe.
    let mut was_local_setup = use_signal(|| local_setup);
    let mut auto_plan = begin_confirmed_plan.clone();
    use_effect(use_reactive(
        (&local_setup, &binding),
        move |(local_setup, binding)| {
            let authoritative = progress.read().clone();
            let page_busy = ops.busy();
            if offer.peek_key().is_some_and(|plan| plan.binding != binding) {
                release_plan_modal(host_id);
                offer.clear();
            }
            let running = authoritative.as_ref().is_some_and(|view| {
                view.run_id.is_some() && view.status == ProvisioningStatus::Running
            });
            if running {
                release_plan_modal(host_id);
                offer.clear();
            }
            if local_setup
                && !page_busy
                && authoritative
                    .as_ref()
                    .is_some_and(|view| view.run_id.is_none())
                && !*local_probe_started.peek()
            {
                local_probe_started.set(true);
                auto_plan(ProvisioningOperation::Add);
            }
            if !local_setup && *was_local_setup.peek() {
                local_probe_started.set(false);
                local_auto_retry.set(false);
                release_plan_modal(host_id);
                offer.clear();
                action_error.set(None);
            }
            if *was_local_setup.peek() != local_setup {
                was_local_setup.set(local_setup);
            }
        },
    ));

    // Publish only the action set. The row owns the menu, but this component
    // remains the authority for when update, rerun, or automatic setup is a
    // truthful offer. Signal reads here keep the summary current without
    // moving any async state into the row.
    let menu_host_kind = host.kind;
    // A host running a newer farhelm than the helm is never offered Update:
    // it would install the helm's older build (`hosts::runs_newer_version`).
    let menu_too_new = crate::hosts::runs_newer_version(&host.state);
    use_effect(use_reactive(
        (&local_setup, &menu_host_kind, &menu_too_new),
        move |(local_setup, menu_host_kind, menu_too_new)| {
            let snapshot = progress();
            let is_planning = planning().is_some();
            let has_pending_plan = offer.is_open();
            let owned = update_owned(intent().as_ref(), tracked().as_ref());
            let can_retry_local_setup = local_auto_retry();
            let run_active = snapshot.as_ref().is_some_and(|view| {
                view.run_id.is_some() && view.status == ProvisioningStatus::Running
            });
            let failed_operation = snapshot.as_ref().and_then(|view| {
                (view.run_id.is_some() && view.status == ProvisioningStatus::Failed)
                    .then_some(view.operation)
                    .flatten()
            });
            let update_allowed = menu_host_kind.offers_provisioning_actions();
            let next = ProvisioningMenuState::offered(MenuFacts {
                run_active,
                failed_operation,
                update_allowed,
                uninstall_allowed: menu_host_kind.uninstalls_from_panel(),
                local_setup,
                confirmation_planning: is_planning,
                has_pending_plan,
                update_owned: owned,
                can_retry_local_setup,
                too_new: menu_too_new,
            });
            if menu_states.peek().get(&host_id).copied() != Some(next) {
                menu_states.write().insert(host_id, next);
            }
        },
    ));

    // An uninstall that failed keeps its host in the list, so the notice
    // owed for its success must not survive to be shown later, for example
    // when the user removes the host by hand. A successful run never gets
    // here: its row, and this panel with it, is gone.
    use_effect(move || {
        let failed_run = progress.read().as_ref().and_then(|view| {
            (view.status == ProvisioningStatus::Failed
                && view.operation == Some(ProvisioningOperation::Uninstall))
            .then(|| view.run_id.clone())
            .flatten()
        });
        let Some(failed_run) = failed_run else {
            return;
        };
        // A retry is confirmed while the row still shows the earlier failed
        // run, and a read of that run can land after the new notice is
        // registered; only a failure of some other run belongs to it.
        let owed_by_another_run = uninstall_notices
            .peek()
            .get(&host_id)
            .is_some_and(|notice| notice.superseded_run.as_deref() != Some(failed_run.as_str()));
        if owed_by_another_run {
            uninstall_notices.write().remove(&host_id);
        }
    });

    // Explicit setup confirmation enters the same binding-checked,
    // single-consumption submission as automatic remote update. ADD
    // preparation may already have registered an answering supervisor, but
    // it never claims this token. The owned guard releases submission
    // exclusion even if this row disappears while the POST is pending.
    let submit_base = base.clone();
    let submit_progress = request_progress.clone();
    // Retain the raw snapshot for the notice's existing peer-rendered name parts.
    let notice_host_name = host.name.raw().to_owned();
    let confirmed_submission_host = host.clone();
    let preference_base = base.clone();
    let host_dialog = settings_dialog::DIALOG;
    // Both explicit cancellations return to this row; confirmation and
    // lifecycle retirement intentionally leave focus to the next surface.
    let row_focus = move || settings_dialog::row_focus(host_id).script();
    // Confirm `shown`, the showing the answer's render drew; the plan it
    // submits is the one that token names (see `take_plan_offer`).
    // `skip_future` is setup's permanent answer ("don't ask in the future"),
    // remembered only once this confirmation has taken its showing: an
    // answer queued behind Cancel, or drawn for a plan since replaced, finds
    // its showing gone and must change no preference either. Every other
    // confirmation passes `false`.
    let confirm = use_callback(move |answer: (ConfirmToken<PendingPlan>, bool)| {
        let (shown, skip_future) = answer;
        let current = current_binding.peek().clone();
        let mut confirm_ops = ops;
        let claim =
            match take_plan_offer(&mut offer, &shown, &current, || confirm_ops.claim_guard()) {
                PlanConfirm::Accepted(claim) => claim,
                PlanConfirm::BindingChanged => {
                    release_plan_modal(host_id);
                    return;
                }
                PlanConfirm::Refused | PlanConfirm::Stale => return,
            };
        let plan = shown.key().clone();
        // The notice an uninstall owes, built now from the plan the user
        // confirmed and registered with the parent BEFORE the POST. Once the
        // host is forgotten nothing else can say where its data stayed, and
        // waiting for the reply is not safe: this panel unmounts with its
        // row, so if the run finishes and the list drops the host before the
        // reply arrives, the submission task dies with the panel and would
        // never register it. A definite refusal withdraws it again.
        if let Some(state_dir) = plan.kept_state_dir.clone() {
            let superseded_run = progress
                .peek()
                .as_ref()
                .and_then(|view| view.run_id.clone());
            uninstall_notices.write().insert(
                host_id,
                UninstallNotice {
                    host_name: notice_host_name.clone(),
                    state_dir,
                    superseded_run,
                },
            );
        }
        // Every attempt consumes what was displayed from the client's point
        // of view too (`take_plan_offer` already took the showing). A
        // refusal or transport ambiguity cannot prove the helm left this
        // one-use id unconsumed.
        release_plan_modal(host_id);
        if skip_future {
            preferences.0.write().skip_host_setup_confirmation = Some(true);
            store_preference(
                &preference_base,
                PreferenceValue::HostSetupConfirmation(true),
            );
        }
        action_error.set(None);
        action_warning.set(None);
        if plan.binding.kind.sets_up_locally() && plan.operation == ProvisioningOperation::Add {
            local_auto_retry.set(false);
        }
        submit_plan(
            Submission {
                base: submit_base.clone(),
                host_id,
                host_snapshot: confirmed_submission_host.clone(),
                submitted_epoch: None,
                claim,
                current_binding,
                intent,
                outstanding: outstanding_submission,
                tracked,
                retired_runs,
                read_gate,
                action_error,
                action_warning,
                update_error,
                update_warning,
                accepted_run_waiting,
                local_auto_retry,
                on_running,
                on_changed,
                uninstall_notices,
                refusals,
                reread: submit_progress.clone(),
            },
            plan,
        );
    });

    // A remembered setup answer applies to failed remote ADD reruns as well
    // as a fresh add. Keep the one-use plan and binding checks in the same
    // submission path; this effect only removes the question from the UI.
    // There is no click to bind here, so the remembered answer carries the
    // token planning got back from `offer.open` and answers exactly that
    // showing. The effect deliberately never reads the slot: reading the
    // open plan here would answer whatever is open when the effect runs, and
    // would subscribe the effect to later openings. A token whose showing
    // reconciliation has since cleared is refused as stale by
    // `take_plan_offer` and confirms nothing.
    use_effect(move || {
        if let Some(shown) = auto_submit_add() {
            auto_submit_add.set(None);
            confirm.call((shown, false));
        }
    });

    let snapshot = progress.read().clone();
    // The showing this render draws. Every answer and dismissal below moves
    // a clone of this token into its handler, so it acts only on this plan.
    let shown = offer.current();
    // ADD and update diagnostics share their render slots: the two
    // lifecycles never run together, so at most one side has news.
    let current_error = action_error
        .read()
        .clone()
        .or_else(|| update_error.read().clone().map(|held| held.text));
    let current_warning = action_warning
        .read()
        .clone()
        .or_else(|| update_warning.read().clone().map(|held| held.text));
    let current_progress_error = progress_error.read().clone();
    let live_intent = intent.read().clone();
    let intent_phase = live_intent.as_ref().map(|live| live.phase);
    let is_planning = planning.read().is_some() || intent_phase == Some(IntentPhase::Planning);
    let is_waiting_claim = intent_phase == Some(IntentPhase::WaitingForClaim);
    let is_submitting = intent_phase == Some(IntentPhase::Submitting);
    let is_accepted_run_waiting = *accepted_run_waiting.read();
    let can_retry_local_setup = *local_auto_retry.read();
    let page_busy = ops.busy();
    // The folded row's one-line trace stands down while the host row shows
    // inline update status (see `derive_row_update_progress`); every other
    // running or failed retained run keeps it. Suppressing it here, where the trace
    // shape is also published, keeps fixed-surface geometry in step with
    // what is actually drawn.
    let inline_update_status = (*planning.read())
        .map(HostUpdateProgress::PendingConfirmed)
        .or_else(|| {
            derive_row_update_progress(
                live_intent.as_ref(),
                tracked.read().as_ref(),
                snapshot.as_ref(),
                confirmed_run_clock.read().as_ref(),
            )
        })
        .is_some();
    let visible_trace = (!details_open && !inline_update_status)
        .then(|| {
            snapshot.as_ref().and_then(|view| {
                let operation = view.operation?;
                matches!(
                    view.status,
                    ProvisioningStatus::Running | ProvisioningStatus::Failed
                )
                .then_some(ProvisioningTraceShape {
                    operation,
                    status: view.status,
                })
            })
        })
        .flatten();
    use_effect(use_reactive((&visible_trace,), move |(visible_trace,)| {
        if let Some(shape) = visible_trace {
            if trace_shapes.peek().get(&host_id).copied() != Some(shape) {
                trace_shapes.write().insert(host_id, shape);
            }
        } else if trace_shapes.peek().contains_key(&host_id) {
            trace_shapes.write().remove(&host_id);
        }
    }));
    let foldable_run = (!global_details_open && details_open)
        .then(|| confirmed_disclosure.read().clone())
        .flatten()
        .filter(|run| !run.folded && !run.failed)
        .map(|run| run.run_id);
    let flow = ProvisioningFlow::resolve(ProvisioningFlowInput {
        snapshot: snapshot.as_ref(),
        offer: shown.as_ref().map(ConfirmToken::key),
        details_open,
        local_setup,
        manual_remedy: manual_remedy.as_deref(),
        is_planning,
        is_waiting_claim,
        is_submitting,
        accepted_run_waiting: is_accepted_run_waiting,
        local_auto_retry: can_retry_local_setup,
        error: current_error.as_deref(),
        warning: current_warning.as_deref(),
        read_error: current_progress_error.as_deref(),
        fold_run: foldable_run.as_deref(),
        visible_trace,
    });
    use_provisioning_flow_movement(&flow, host_id, capture_lower_hosts);

    // Every dismissal the confirmations below draw: Cancel, the modal
    // header's dismiss, and Escape. It acts only for `shown`, the showing it
    // was drawn for: the `take` closes the question only if that showing is
    // still open, and the rest of the dismissal runs only then. A dismissal
    // drawn for an earlier showing, or queued behind a confirmation in the
    // same event burst, must not close, pull focus out of, or wipe the
    // diagnostics of the question now on screen (or of the run a
    // confirmation just submitted), nor re-arm the local setup retry.
    // `close_dialog` is whether the question is a modal whose isolation to
    // release, with focus back on this row; `retry_local_setup` re-arms a
    // local row's automatic setup retry, as Setup's and the inline Update's
    // dismissals always have.
    let mut dismiss =
        move |shown: &ConfirmToken<PendingPlan>, close_dialog: bool, retry_local_setup: bool| {
            if offer.take(shown).is_none() {
                return;
            }
            if close_dialog {
                host_dialog.close(&row_focus());
            }
            action_error.set(None);
            action_warning.set(None);
            if retry_local_setup && local_setup {
                local_auto_retry.set(true);
            }
        };

    rsx! {
        div {
            class: "provisioning-panel",
            "data-provisioning-host": "{host_id}",
            // Confirmation modals live outside row disclosure: opening one
            // must not switch on the global details checkbox.
            if let ProvisioningFlow::Body(body) = flow {
            div {
            if let Some(run) = body.run {
                div {
                        class: "provisioning-run",
                        "data-provisioning-status": "{run.status}",
                        "data-provisioning-operation": "{run.operation}",
                        div { class: "provisioning-run-header",
                            span { class: "provisioning-title",
                                "{run.operation} provisioning"
                            }
                            span { class: "provisioning-status", "{run.status}" }
                            if let Some(fold_run_id) = run.fold_run {
                                button {
                                    r#type: "button",
                                    class: "btn provisioning-run-fold",
                                    "data-tooltip": "collapse this run's steps",
                                    aria_label: "collapse this run's steps",
                                    onclick: move |_| {
                                        let observed = confirmed_disclosure.peek().clone();
                                        if let Some(current) = observed
                                            && current.run_id == fold_run_id && !current.failed
                                        {
                                            confirmed_disclosure.set(Some(ConfirmedRunDisclosure {
                                                folded: true,
                                                ..current
                                            }));
                                        }
                                    },
                                    "collapse"
                                }
                            }
                        }
                        ol { class: "provisioning-steps",
                            for step in run.steps {
                                li {
                                    class: "provisioning-step",
                                    "data-step": "{step.id}",
                                    "data-status": "{step.raw_status}",
                                    "data-current": if step.raw_status == "running" { "true" },
                                    span { class: "provisioning-step-marker", aria_hidden: "true" }
                                    span { class: "provisioning-step-name peer-value", dir: "ltr", "{step.name}" }
                                    span { class: "provisioning-step-status", "{step.status}" }
                                    if let Some(message) = step.message {
                                        PeerLine {
                                            class: "provisioning-step-message",
                                            parts: message.0,
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(message) = run.message {
                            PeerLine {
                                class: "provisioning-run-message",
                                parts: message.0,
                            }
                        }
                }
            }

            if let Some(shown) = shown {
                if shown.key().operation == ProvisioningOperation::Add {
                    crate::modal_isolation::HostModal { key: "{shown.key().probe_id}",
                    div { class: "host-settings-backdrop", role: "presentation",
                        ModalDialog {
                            handle: DialogHandle::new(settings_dialog::ADD_DIALOG_SELECTOR),
                            focus: DialogFocus::remembered(
                                ".add-host-ssh", Some(".add-host-cancel"), false,
                                Some(".provisioning-confirm, .add-host-submit"),
                            ),
                            // Setup first focuses the root and isolates it, then
                            // hands focus to the quiet confirming answer. Keep
                            // that handoff after the shared gap/Enter install.
                            after_install_focus: FocusTarget::new(
                                ".host-add-dialog .provisioning-confirm:enabled",
                                Some(".add-host-cancel"), true,
                            ),
                            class: "host-settings-dialog host-add-dialog host-dialog-framed",
                            role: "dialog",
                            aria_modal: "true",
                            aria_label: "host setup",
                            tabindex: "-1",
                            // Cancel and Escape stay live while another
                            // operation holds the page token: dismissing the
                            // question starts nothing, and the confirm path
                            // takes its showing before it submits, so there
                            // is no request of this row's to strand.
                            on_escape: {
                                let shown = shown.clone();
                                move |evt: KeyboardEvent| {
                                    if !evt.is_composing() {
                                        dismiss(&shown, true, true);
                                    }
                                }
                            },
                            SetupPlanConfirmation {
                                confirmation: shown.key().confirmation.clone(),
                                steps: shown.key().steps.clone(),
                                name: crate::hosts::gui_host_name(host.name.raw(), host.kind == HostKind::Local),
                                host: Some(host.clone()),
                                local: host.kind == HostKind::Local,
                                busy: page_busy,
                                // The permanent answer is remembered inside
                                // `confirm`, once it has taken this showing.
                                on_confirm: {
                                    let shown = shown.clone();
                                    move |skip_future| confirm.call((shown.clone(), skip_future))
                                },
                                on_cancel: {
                                    let shown = shown.clone();
                                    move |_| dismiss(&shown, true, true)
                                },
                            }
                        }
                    }
                    }
                } else if shown.key().operation == ProvisioningOperation::Uninstall {
                    crate::modal_isolation::HostModal { key: "{shown.key().probe_id}",
                    // A removal the user has to read before agreeing to,
                    // so it gets the same modal treatment as setup, with
                    // the destructive confirm in the danger tier. Cancel
                    // and Escape stay live for the reason the setup
                    // dialog's do.
                    div { class: "host-settings-backdrop", role: "presentation",
                        ModalDialog {
                            handle: DialogHandle::new(settings_dialog::UNINSTALL_DIALOG_SELECTOR),
                            focus: DialogFocus::remembered(
                                ".uninstall-confirm:enabled", Some(".uninstall-cancel"), true,
                                Some(".uninstall-confirm"),
                            ),
                            class: "host-settings-dialog host-uninstall-dialog host-dialog-framed",
                            role: "dialog",
                            aria_modal: "true",
                            aria_label: "uninstall Farhelm from this host",
                            tabindex: "-1",
                            on_escape: {
                                let shown = shown.clone();
                                move |evt: KeyboardEvent| {
                                    if !evt.is_composing() {
                                        dismiss(&shown, true, false);
                                    }
                                }
                            },
                            UninstallConfirmation {
                                confirmation: shown.key().confirmation.clone(),
                                steps: shown.key().steps.clone(),
                                host: host.clone(),
                                busy: page_busy,
                                on_confirm: {
                                    let shown = shown.clone();
                                    move |_| confirm.call((shown.clone(), false))
                                },
                                on_cancel: {
                                    let shown = shown.clone();
                                    move |_| dismiss(&shown, true, false)
                                },
                            }
                        }
                    }
                    }
                } else if let Some(inline_plan) = body.inline_plan {
                    PlanConfirmation {
                        confirmation: shown.key().confirmation.clone(),
                        body: inline_plan,
                        busy: page_busy,
                        confirm_label: "confirm update",
                        on_confirm: {
                            let shown = shown.clone();
                            move |_| confirm.call((shown.clone(), false))
                        },
                        // The offer owns no submitted request or page token;
                        // cancellation must stay available while a peer is busy.
                        on_cancel: {
                            let shown = shown.clone();
                            move |_| dismiss(&shown, false, true)
                        },
                    }
                }
            } else if let Some(pending_line) = body.pending {
                div {
                    class: pending_line.class(),
                    "{pending_line.text()}"
                }
            }

            if let Some(error) = body.error {
                PeerLine {
                    class: "action-error provisioning-error",
                    parts: error.0,
                }
            }
            if let Some(warning) = body.warning {
                PeerLine {
                    class: "host-warning provisioning-warning",
                    parts: warning.0,
                }
            }
            if let Some(error) = body.read_error {
                PeerLine {
                    class: "action-error provisioning-read-error",
                    parts: error.0,
                }
            }
            if let Some(remedy) = body.manual {
                PeerLine {
                    class: if remedy.secondary {
                        "host-remedy provisioning-manual secondary"
                    } else {
                        "host-remedy provisioning-manual"
                    },
                    parts: remedy.parts.0,
                }
            }
            }
            } else if let ProvisioningFlow::Trace(shape) = flow {
                div {
                    class: "provisioning-trace",
                    "data-provisioning-status": "{status_label(shape.status)}",
                    "{operation_label(Some(shape.operation))} provisioning {status_label(shape.status)}"
                }
            }
        }
    }
}

/// Stable words for aggregate run state, also used as browser handles.
fn status_label(status: ProvisioningStatus) -> &'static str {
    match status {
        ProvisioningStatus::Running => "running",
        ProvisioningStatus::Completed => "completed",
        ProvisioningStatus::Failed => "failed",
    }
}

/// The operation label for a retained run; `None` occurs only in the idle
/// view, which the panel does not render as a run.
fn operation_label(operation: Option<ProvisioningOperation>) -> &'static str {
    match operation {
        Some(ProvisioningOperation::Add) => "setup",
        Some(ProvisioningOperation::Update) => "update",
        Some(ProvisioningOperation::Uninstall) => "uninstall",
        None => "idle",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchored_popovers::{AnchorLease, AnchorRegion, Interest, MovedRows, MovementOwner};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    /// A named step prevents the compatibility id fallback from confusing
    /// metadata-only changes with actual name changes in these tests.
    fn flow_view() -> ProvisioningView {
        ProvisioningView {
            run_id: Some("run-a".into()),
            operation: Some(ProvisioningOperation::Update),
            status: ProvisioningStatus::Running,
            steps: vec![crate::api::ProvisioningStep {
                step: "step-a".into(),
                name: Some("upload\u{200b}build".into()),
                status: "running".into(),
                message: Some("message\u{202e}".into()),
                ..Default::default()
            }],
            message: Some("run\u{2066}".into()),
        }
    }

    /// Render-only fixtures need an offer's normal identity without any
    /// request, runtime scope, page lock or mutable process environment.
    fn flow_plan(operation: ProvisioningOperation) -> PendingPlan {
        PendingPlan {
            operation,
            binding: HostBinding {
                id: 1,
                kind: HostKind::Ssh,
                destination: None,
                identity: None,
                remote_farhelm: None,
                remote_state_dir: None,
                incarnation: 0,
            },
            probe_id: "plan-a".into(),
            confirmation: "legacy plan".into(),
            steps: None,
            kept_state_dir: None,
        }
    }

    /// Hold one ordinary admitted lower-host opening while exercising the
    /// production provisioning projection. Host vector selection, measurement
    /// and generic admission interleavings have their own tests; this fixture
    /// neither simulates a rectangle nor starts provisioning transport.
    struct FlowMovementFixture {
        flow: RefCell<ProvisioningFlow>,
        owner: MovementOwner,
        open: Rc<RefCell<Option<String>>>,
        closes: Rc<Cell<usize>>,
        captured_sources: RefCell<Vec<HostId>>,
        scope: Cell<Option<dioxus::core::ScopeId>>,
        _opening: AnchorLease,
    }

    impl FlowMovementFixture {
        /// Establish the opening before mount so an erroneous initial capture
        /// cannot hide behind a closed slot. The lease stays live throughout
        /// these producer tests; no admission policy is reproduced here.
        fn new(flow: ProvisioningFlow) -> Rc<Self> {
            let owner = MovementOwner::default();
            let open = Rc::new(RefCell::new(Some("lower".to_owned())));
            let closes = Rc::new(Cell::new(0));
            let current = open.clone();
            let closing = open.clone();
            let delivered = closes.clone();
            let registration = owner.register(
                Interest::new(AnchorRegion::Hosts),
                move || current.borrow().clone(),
                move || {
                    delivered.set(delivered.get() + 1);
                    *closing.borrow_mut() = None;
                },
            );
            let opening = registration
                .bind_anchor("lower".into(), || true)
                .expect("the fixture lower-host opening must be admitted");
            Rc::new(Self {
                flow: RefCell::new(flow),
                owner,
                open,
                closes,
                captured_sources: RefCell::new(Vec::new()),
                scope: Cell::new(None),
                _opening: opening,
            })
        }

        /// Mount the real production hook helper and drain only its initial
        /// effect. The probe has no async readers or application contexts.
        fn mount(self: &Rc<Self>) -> VirtualDom {
            let mut dom = VirtualDom::new_with_props(provisioning_flow_probe, self.clone());
            dom.rebuild_to_vec();
            dom.process_events();
            assert!(
                self.captured_sources.borrow().is_empty(),
                "mount seeds the baseline"
            );
            assert_eq!(self.closes.get(), 0);
            assert_eq!(self.open.borrow().as_deref(), Some("lower"));
            dom
        }

        /// Publish one changed render while deliberately leaving its deferred
        /// effect pending. Returned edits prove the component actually reran;
        /// capture and delivery can then be observed at distinct boundaries.
        fn render(
            &self,
            dom: &mut VirtualDom,
            flow: ProvisioningFlow,
        ) -> Vec<dioxus::core::Mutation> {
            self.flow.replace(flow);
            dom.mark_dirty(self.scope.get().expect("producer mounted"));
            dom.render_immediate_to_vec().edits
        }
    }

    /// Exercise the exact helper ProvisioningPanel calls before its RSX.
    /// Debug text is only a render witness: this proves production projection
    /// and capture timing, while the browser fixture covers the actual panel's
    /// markup, transport and physical anchor movement.
    fn provisioning_flow_probe(fixture: Rc<FlowMovementFixture>) -> Element {
        fixture.scope.set(Some(dioxus::core::current_scope_id()));
        let capturing = fixture.clone();
        let capture_lower_hosts = use_callback(move |source: HostId| {
            assert!(
                capturing.flow.try_borrow_mut().is_ok(),
                "capture holds no input borrow"
            );
            capturing.captured_sources.borrow_mut().push(source);
            capturing
                .owner
                .capture_affected(&MovedRows::new(AnchorRegion::Hosts, ["lower".into()]))
        });
        let flow = fixture.flow.borrow().clone();
        use_provisioning_flow_movement(&flow, 41, capture_lower_hosts);
        rsx! { span { "{flow:?}" } }
    }

    /// Empty/Trace renders stay on the existing trace route. A later first
    /// Body still captures a lower opening, including the real occupancy of
    /// a body whose only offer is a fixed-position modal.
    #[farhelm_testtrace::test]
    fn provisioning_producer_seeds_then_captures_the_first_empty_body() {
        let fixture = FlowMovementFixture::new(ProvisioningFlow::Empty);
        let mut dom = fixture.mount();
        for flow in [
            ProvisioningFlow::Empty,
            ProvisioningFlow::Trace(ProvisioningTraceShape {
                operation: ProvisioningOperation::Update,
                status: ProvisioningStatus::Running,
            }),
            ProvisioningFlow::Trace(ProvisioningTraceShape {
                operation: ProvisioningOperation::Update,
                status: ProvisioningStatus::Failed,
            }),
            ProvisioningFlow::Empty,
        ] {
            fixture.render(&mut dom, flow);
            dom.process_events();
        }
        assert!(fixture.captured_sources.borrow().is_empty());
        assert_eq!(fixture.closes.get(), 0);
        let plan = flow_plan(ProvisioningOperation::Add);
        let body = ProvisioningFlow::resolve(ProvisioningFlowInput {
            offer: Some(&plan),
            ..Default::default()
        });
        fixture.render(&mut dom, body);
        assert_eq!(*fixture.captured_sources.borrow(), vec![41]);
        assert_eq!(
            fixture.closes.get(),
            0,
            "capture precedes deferred delivery"
        );
        dom.process_events();
        assert_eq!(fixture.closes.get(), 1);
        assert!(fixture.open.borrow().is_none());
    }

    /// An unchanged visible body must not capture on retained metadata churn.
    /// A later wrapped-message change captures during the committed render,
    /// before the shared continuation delivers to the admitted lower opening.
    #[farhelm_testtrace::test]
    fn provisioning_producer_ignores_metadata_and_defers_changed_body_delivery() {
        let mut view = flow_view();
        let resolve = |view: &ProvisioningView| {
            ProvisioningFlow::resolve(ProvisioningFlowInput {
                snapshot: Some(view),
                details_open: true,
                ..Default::default()
            })
        };
        let fixture = FlowMovementFixture::new(resolve(&view));
        let mut dom = fixture.mount();
        fixture.render(&mut dom, resolve(&view));
        view.run_id = Some("metadata-only-run".into());
        view.steps[0].step = "metadata-only-step".into();
        view.steps[0].explanation = Some("hidden explanation".into());
        let edits = fixture.render(&mut dom, resolve(&view));
        assert!(
            edits.iter().any(|edit| matches!(edit,
            dioxus::core::Mutation::SetText { value, .. }
                if value.contains("metadata-only-step"))),
            "the metadata render really committed"
        );
        dom.process_events();
        assert!(fixture.captured_sources.borrow().is_empty());
        assert_eq!(fixture.closes.get(), 0);
        view.steps[0].message = Some("a changed wrapped message".into());
        let edits = fixture.render(&mut dom, resolve(&view));
        assert!(
            edits.iter().any(|edit| matches!(edit,
            dioxus::core::Mutation::SetText { value, .. }
                if value.contains("a changed wrapped message"))),
            "the changed body render committed"
        );
        assert_eq!(*fixture.captured_sources.borrow(), vec![41]);
        assert_eq!(
            fixture.closes.get(),
            0,
            "the post-render continuation has not run"
        );
        dom.process_events();
        assert_eq!(fixture.closes.get(), 1);
    }

    /// Removing visible content can move a lower row as surely as adding it.
    /// Body-to-Trace must report that removal even though later Trace-only
    /// transitions remain delegated to the original trace producer.
    #[farhelm_testtrace::test]
    fn provisioning_producer_captures_body_removal() {
        let flow = ProvisioningFlow::resolve(ProvisioningFlowInput {
            details_open: true,
            error: Some("visible diagnostic"),
            ..Default::default()
        });
        let fixture = FlowMovementFixture::new(flow);
        let mut dom = fixture.mount();
        fixture.render(
            &mut dom,
            ProvisioningFlow::Trace(ProvisioningTraceShape {
                operation: ProvisioningOperation::Update,
                status: ProvisioningStatus::Running,
            }),
        );
        assert_eq!(*fixture.captured_sources.borrow(), vec![41]);
        assert_eq!(fixture.closes.get(), 0);
        dom.process_events();
        assert_eq!(fixture.closes.get(), 1);
    }

    /// These independently accepted inputs all change the production Body,
    /// even when no retained run changes its trace summary. Exercise the
    /// actual projection and hook together: a first read refusal, manual
    /// advice, a pending-stage transition and the defensive inline plan must
    /// capture before deferred delivery rather than depend on panel resize.
    #[farhelm_testtrace::test]
    fn provisioning_producer_reports_diagnostic_manual_pending_and_plan_changes() {
        let read_error = ProvisioningFlow::resolve(ProvisioningFlowInput {
            details_open: true,
            read_error: Some("first accepted read refusal"),
            ..Default::default()
        });
        let manual_short = vec![DetailPart::peer("run the supervisor")];
        let manual_long = vec![DetailPart::peer(
            "run the supervisor with a longer state directory",
        )];
        let manual = |parts| {
            ProvisioningFlow::resolve(ProvisioningFlowInput {
                details_open: true,
                local_setup: true,
                manual_remedy: Some(parts),
                ..Default::default()
            })
        };
        let waiting = ProvisioningFlow::resolve(ProvisioningFlowInput {
            details_open: true,
            is_waiting_claim: true,
            ..Default::default()
        });
        let planning = ProvisioningFlow::resolve(ProvisioningFlowInput {
            details_open: true,
            is_planning: true,
            ..Default::default()
        });
        let mut plan = flow_plan(ProvisioningOperation::Update);
        let original_plan = ProvisioningFlow::resolve(ProvisioningFlowInput {
            offer: Some(&plan),
            ..Default::default()
        });
        plan.confirmation = "changed defensive inline plan".into();
        let changed_plan = ProvisioningFlow::resolve(ProvisioningFlowInput {
            offer: Some(&plan),
            ..Default::default()
        });

        for (cause, before, after) in [
            ("first read refusal", ProvisioningFlow::Empty, read_error),
            ("manual advice", manual(&manual_short), manual(&manual_long)),
            ("pending stage", waiting, planning),
            ("inline plan", original_plan, changed_plan),
        ] {
            assert_ne!(before, after, "{cause} must change actual rendered flow");
            let fixture = FlowMovementFixture::new(before);
            let mut dom = fixture.mount();
            let edits = fixture.render(&mut dom, after);
            assert!(!edits.is_empty(), "{cause} must commit its changed render");
            assert_eq!(*fixture.captured_sources.borrow(), vec![41], "{cause}");
            assert_eq!(fixture.closes.get(), 0, "{cause}: delivery is pending");
            dom.process_events();
            assert_eq!(fixture.closes.get(), 1, "{cause}");
            assert!(fixture.open.borrow().is_none(), "{cause}");
        }
    }

    /// Folded payloads are invisible, but an offer's inner wrapper occupies
    /// flow even when only its fixed-position modal is rendered. A later
    /// first progress response must differ from the seeded empty baseline.
    #[farhelm_testtrace::test]
    fn flow_preserves_empty_body_occupancy_and_first_expanded_reply() {
        let view = flow_view();
        let empty = ProvisioningFlow::resolve(ProvisioningFlowInput::default());
        assert_eq!(empty, ProvisioningFlow::Empty);
        assert_eq!(
            empty,
            ProvisioningFlow::resolve(ProvisioningFlowInput {
                snapshot: Some(&view),
                ..Default::default()
            })
        );
        let mut plan = flow_plan(ProvisioningOperation::Add);
        let body = ProvisioningFlow::resolve(ProvisioningFlowInput {
            offer: Some(&plan),
            ..Default::default()
        });
        assert!(matches!(body, ProvisioningFlow::Body(_)));
        assert_ne!(empty, body);
        plan.probe_id = "new-plan".into();
        plan.confirmation = "different modal words".into();
        assert_eq!(
            body,
            ProvisioningFlow::resolve(ProvisioningFlowInput {
                offer: Some(&plan),
                ..Default::default()
            })
        );
        assert_ne!(
            empty,
            ProvisioningFlow::resolve(ProvisioningFlowInput {
                snapshot: Some(&view),
                details_open: true,
                ..Default::default()
            })
        );
    }

    /// Equality follows the expanded words and controls actually rendered.
    /// Run identity, CSS/browser step handles, hidden explanation and paths
    /// must not turn an unchanged row into a new movement cause.
    #[farhelm_testtrace::test]
    fn expanded_flow_compares_displayed_text_and_fold_presence() {
        let mut view = flow_view();
        let resolve = |view: &ProvisioningView, fold_run: Option<&str>| {
            ProvisioningFlow::resolve(ProvisioningFlowInput {
                snapshot: Some(view),
                details_open: true,
                fold_run,
                ..Default::default()
            })
        };
        let original = resolve(&view, Some("run-a"));
        let ProvisioningFlow::Body(body) = &original else {
            panic!("expected expanded body")
        };
        let run = body.run.as_ref().unwrap();
        assert_eq!(run.steps[0].name, "upload<U+200B>build");
        assert_eq!(run.steps[0].status, "running");
        assert_eq!(run.steps[0].id, "step-a");
        assert_eq!(run.steps[0].raw_status, "running");
        assert_eq!(
            run.steps[0].message.as_ref().unwrap().0,
            vec![DetailPart::peer("message\u{202e}")]
        );
        view.run_id = Some("run-b".into());
        view.steps[0].step = "different-id".into();
        view.steps[0].explanation = Some("hover-only explanation".into());
        assert_eq!(original, resolve(&view, Some("run-b")));
        assert_ne!(original, resolve(&view, None));
        view.steps[0].status = "completed".into();
        assert_ne!(original, resolve(&view, Some("run-b")));
        view.steps[0].status = "running".into();
        view.steps[0].message = Some("changed wrapped message".into());
        assert_ne!(original, resolve(&view, Some("run-b")));
        view.steps[0].message = Some("message\u{202e}".into());
        view.steps[0].name = Some("".into());
        assert_ne!(original, resolve(&view, Some("run-b")));
    }

    /// Advice slots and the manual remedy can rewrap while disclosure stays
    /// open. Secondary advice also changes margin; neither distinction may
    /// disappear when the render branches are extracted.
    #[farhelm_testtrace::test]
    fn flow_keeps_diagnostic_slots_and_manual_secondary_margin() {
        let remedy = vec![
            DetailPart::text("run "),
            DetailPart::peer("command\u{200b}"),
        ];
        let resolve = |retry, error| {
            ProvisioningFlow::resolve(ProvisioningFlowInput {
                details_open: true,
                local_setup: true,
                manual_remedy: Some(&remedy),
                local_auto_retry: retry,
                error: Some(error),
                warning: Some("warning"),
                read_error: Some("read failed"),
                ..Default::default()
            })
        };
        let primary = resolve(false, "error");
        let secondary = resolve(true, "error");
        assert_ne!(primary, secondary);
        assert_ne!(primary, resolve(false, "a longer error"));
        let ProvisioningFlow::Body(body) = secondary else {
            panic!("expected advice body")
        };
        assert_eq!(body.error, Some(FlowParts(vec![DetailPart::peer("error")])));
        assert_eq!(
            body.warning,
            Some(FlowParts(vec![DetailPart::peer("warning")]))
        );
        assert_eq!(
            body.read_error,
            Some(FlowParts(vec![DetailPart::peer("read failed")]))
        );
        assert_eq!(
            body.manual,
            Some(ManualLine {
                parts: FlowParts(remedy),
                secondary: true
            })
        );
        assert_ne!(
            FlowParts(vec![DetailPart::text("peer")]),
            FlowParts(vec![DetailPart::peer("peer")])
        );
        assert_eq!(
            FlowParts(vec![DetailPart::peer("\u{200b}")]),
            FlowParts(vec![DetailPart::peer("<U+200B>")])
        );
    }

    /// An offer suppresses all pending-stage lines even while Details is
    /// folded. The defensive Update offer is an inline plan, so its words
    /// participate where Add and Uninstall modal words do not.
    #[farhelm_testtrace::test]
    fn flow_resolves_offer_precedence_and_defensive_inline_plan() {
        let mut plan = flow_plan(ProvisioningOperation::Update);
        /// Resolve each snapshot with a call-local borrow: the fixture edits
        /// the same plan between two owned presentation values.
        fn resolve(offer: Option<&PendingPlan>) -> ProvisioningFlow {
            ProvisioningFlow::resolve(ProvisioningFlowInput {
                offer,
                details_open: true,
                is_planning: true,
                is_waiting_claim: true,
                is_submitting: true,
                ..Default::default()
            })
        }
        let planning = resolve(None);
        let ProvisioningFlow::Body(body) = planning else {
            panic!("expected planning body")
        };
        assert_eq!(body.pending, Some(PendingLine::Planning));
        let offered = resolve(Some(&plan));
        let ProvisioningFlow::Body(body) = &offered else {
            panic!("expected offer body")
        };
        assert!(body.pending.is_none());
        assert_eq!(
            body.inline_plan,
            Some(PlanBody::Legacy("legacy plan".into()))
        );
        plan.confirmation = "changed inline plan".into();
        assert_ne!(offered, resolve(Some(&plan)));
    }

    /// Plan flow equality excludes Copy bytes, DOM keys and full hover paths
    /// while strict props still update those render details. Legacy equality
    /// shares the actual line renderer's normalization and keeps terminators
    /// structural rather than flattening them into one escaped label.
    #[farhelm_testtrace::test]
    fn plan_body_excludes_copy_and_hover_metadata() {
        let mut steps: PlanSteps = serde_json::from_value(serde_json::json!({
            "lead": "lead\u{200b}",
            "steps": [{
                "id": "step-a", "name": "name", "paths": [{"short": "~/a", "full": "/host/a"}],
                "explanation": "why"
            }],
            "kept": "kept"
        }))
        .unwrap();
        let original = PlanBody::new("raw copy a", Some(&steps));
        let resolve = |steps: &PlanSteps| {
            let mut plan = flow_plan(ProvisioningOperation::Update);
            plan.steps = Some(steps.clone());
            ProvisioningFlow::resolve(ProvisioningFlowInput {
                offer: Some(&plan),
                ..Default::default()
            })
        };
        let original_flow = resolve(&steps);
        steps.steps[0].id = "step-b".into();
        steps.steps[0].paths[0].full = "/other/full/path".into();
        let metadata_changed = PlanBody::new("different copy bytes", Some(&steps));
        assert_ne!(original, metadata_changed);
        assert!(original.same_flow(&metadata_changed));
        assert_eq!(original_flow, resolve(&steps));
        steps.steps[0].paths[0].short = "~/different".into();
        assert!(!original.same_flow(&PlanBody::new("raw copy a", Some(&steps))));
        assert_ne!(original_flow, resolve(&steps));
        for (left, right) in [
            ("line\r\n- step\r\n", "line\n- step\n"),
            ("peer\u{202e}", "peer<U+202E>"),
        ] {
            let left = PlanBody::new(left, None);
            let right = PlanBody::new(right, None);
            assert_ne!(left, right, "raw legacy props still change");
            assert!(
                left.same_flow(&right),
                "identical displayed legacy bodies do not move"
            );
        }
        assert!(!PlanBody::new("line\n", None).same_flow(&PlanBody::new("line", None)));
        assert!(!PlanBody::new("line\n", None).same_flow(&PlanBody::new("line<U+000A>", None)));
    }

    /// A decoded plan must keep its copyable bytes while both production
    /// renderers show hostile text visibly. The fallback also preserves blank
    /// lines and spaces instead of inventing the single-line placeholders.
    #[farhelm_testtrace::test]
    fn decoded_plan_text_keeps_raw_bytes_and_escapes_rendered_values() {
        /// Mount the production renderer with the local confirmation snapshot;
        /// neither branch gets a test-only presentation path.
        fn render((confirmation, steps): (String, Option<PlanSteps>)) -> Element {
            let body = PlanBody::new(&confirmation, steps.as_ref());
            rsx! { PlanStepList { confirmation, body } }
        }

        let raw = "heading\r\n- remove path\u{202e}\r\n   \r\n\t\n";
        let plan: crate::api::UninstallPlan = serde_json::from_value(serde_json::json!({
            "probe_id": "opaque-plan",
            "confirmation": raw,
            "steps": {
                "lead": "target\u{200b}",
                "steps": [{
                    "id": "remove-unit",
                    "name": "remove\u{202e}",
                    "paths": [{"full": "/home/host/\u{200b}unit", "short": "~/\u{200b}unit"}],
                    "explanation": "reason\u{2066}"
                }],
                "kept": "keep\u{200b}"
            },
            "plan": {"state_dir": "/home/host/\u{200b}data"}
        }))
        .unwrap();
        assert_eq!(plan.probe_id, "opaque-plan");
        assert_eq!(plan.confirmation.raw(), raw);
        assert_eq!(plan.plan.state_dir.raw(), "/home/host/\u{200b}data");

        for (steps, expected) in [
            (
                plan.steps,
                vec![
                    "target<U+200B>",
                    "remove<U+202E>",
                    "~/<U+200B>unit",
                    "reason<U+2066>",
                    "keep<U+200B>",
                ],
            ),
            (
                None,
                vec!["heading", "- remove path<U+202E>", "   ", "<U+0009>"],
            ),
        ] {
            // This is the same raw local snapshot the confirmation and Copy
            // plan handlers receive; rendering must never rewrite its source.
            let confirmation = plan.confirmation.raw().to_owned();
            assert_eq!(confirmation.as_bytes(), raw.as_bytes());
            let structured = steps.is_some();
            let mut dom = VirtualDom::new_with_props(render, (confirmation, steps));
            let edits = dom.rebuild_to_vec().edits;
            let texts = edits
                .iter()
                .filter_map(|edit| match edit {
                    dioxus::core::Mutation::CreateTextNode { value, .. }
                    | dioxus::core::Mutation::SetText { value, .. } => Some(value.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            for expected in expected {
                assert!(texts.contains(&expected), "missing {expected:?}: {texts:?}");
            }
            if structured {
                assert!(edits.iter().any(|edit| matches!(edit,
                    dioxus::core::Mutation::SetAttribute {
                        name: "data-tooltip",
                        value: dioxus::core::AttributeValue::Text(value),
                        ..
                    } if value == "/home/host/<U+200B>unit"
                )));
            }
            assert!(
                texts
                    .iter()
                    .all(|text| !text.contains(['\u{200b}', '\u{202e}', '\u{2066}', '\r', '\t']))
            );
            assert!(!texts.iter().any(|text| text.contains("whitespace only")));
        }
    }

    /// Names and diagnostics remain peer data even when copied into the
    /// folded row's local summary. Missing and empty names fall back to the
    /// stable step label; whitespace and unknown finite status words do not.
    #[farhelm_testtrace::test]
    fn decoded_progress_keeps_name_fallbacks_and_raw_summary_diagnostics() {
        let view: ProvisioningView = serde_json::from_value(serde_json::json!({
            "run_id": "opaque-run",
            "operation": "update",
            "status": "running",
            "steps": [{
                "step": "upload-farhelm",
                "name": " \t ",
                "paths": [{"full": "/host/\u{200b}path", "short": "~/\u{200b}path"}],
                "explanation": "reason\u{202e}",
                "status": "future-status",
                "message": "command\u{200b}\r\nfailed"
            }],
            "message": "aggregate\u{2066}"
        }))
        .unwrap();
        let summary = summarize_running_view(
            &view,
            "opaque-run".to_owned(),
            Instant::now(),
            ProvisioningOperation::Update,
        );
        assert_eq!(summary.run_id, "opaque-run");
        assert_eq!(summary.steps[0].id, "upload-farhelm");
        assert_eq!(summary.steps[0].name, " \t ");
        assert_eq!(summary.steps[0].status, "future-status");
        assert_eq!(
            summary.steps[0].message.as_deref(),
            Some("command\u{200b}\r\nfailed")
        );
        assert_eq!(view.message.as_ref().unwrap().raw(), "aggregate\u{2066}");
        let mut step = view.steps[0].clone();
        assert_eq!(progress_step_name(&step), " \t ");
        assert_eq!(progress_step_status(&step.status), "future-status");
        assert_eq!(step.paths[0].full.raw(), "/host/\u{200b}path");
        assert_eq!(step.explanation.as_ref().unwrap().raw(), "reason\u{202e}");
        assert_eq!(
            step.message.as_ref().unwrap().raw(),
            "command\u{200b}\r\nfailed"
        );
        step.name = Some("name\u{200b}".into());
        assert_eq!(display_peer(progress_step_name(&step)), "name<U+200B>");
        step.name = Some("".into());
        assert_eq!(progress_step_name(&step), "upload-farhelm");
        step.name = None;
        assert_eq!(progress_step_name(&step), "upload-farhelm");
    }

    /// A requested plan's failure opens the matching modal, while the
    /// trusted Remove marker survives only uninstall planning. This pins
    /// the client routing separately from the helm's header producer.
    #[farhelm_testtrace::test]
    fn planning_failures_keep_their_action_and_narrow_remedy() {
        for (operation, action) in [
            (ProvisioningOperation::Add, HostRefusalAction::Setup),
            (ProvisioningOperation::Update, HostRefusalAction::Update),
            (
                ProvisioningOperation::Uninstall,
                HostRefusalAction::Uninstall,
            ),
        ] {
            let refusal = planning_refusal(operation, "planning failed".to_string().into());
            assert_eq!(refusal.action, action);
            assert_eq!(refusal.reason, "planning failed");
            assert!(!refusal.remove_instead);
        }
        let typed = crate::api::UninstallPlanError {
            reason: "remove it from the list".into(),
            remove_instead: true,
        };
        let refusal = planning_refusal(ProvisioningOperation::Uninstall, typed.into());
        assert!(refusal.remove_instead);
    }

    /// Mount a plan-confirmation slot the way `ProvisioningPanel` does and
    /// run `body` against it inside the runtime, so the tests below exercise
    /// the real signal-backed slot rather than a stand-in.
    fn with_plan_slot(body: impl FnOnce(ConfirmSlot<PendingPlan>)) {
        std::thread_local! {
            static SLOT: Cell<Option<ConfirmSlot<PendingPlan>>> = const { Cell::new(None) };
        }

        fn app() -> Element {
            let slot = use_confirm_slot::<PendingPlan, ()>();
            SLOT.with(|cell| cell.set(Some(slot)));
            rsx! {}
        }

        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        let slot = SLOT.with(Cell::get).expect("the slot mounted");
        dom.in_runtime(|| body(slot));
    }

    /// What a test's page-lock stand-in has seen: how many claims were ever
    /// made, and how many are still held.
    ///
    /// Both counts are needed. `made` is what tells an answer that never
    /// touched the lock from one that claimed it and dropped the claim again:
    /// both leave `held` at zero, but only the second wrote the lock (waking
    /// its readers) for an answer that could not land. `held` shows the
    /// accepted answer's claim is the one that survives into the submission.
    #[derive(Debug, Default, PartialEq)]
    struct ClaimLedger {
        made: Cell<u32>,
        held: Cell<i32>,
    }

    /// A page-lock stand-in for [`take_plan_offer`]: one held claim in its
    /// [`ClaimLedger`] until dropped.
    #[derive(Debug, PartialEq)]
    struct CountedClaim(Rc<ClaimLedger>);

    impl Drop for CountedClaim {
        fn drop(&mut self) {
            self.0.held.set(self.0.held.get() - 1);
        }
    }

    /// A claim source over `ledger` for [`take_plan_offer`]: every call
    /// succeeds, and is recorded as one more claim made and held.
    fn counted_claim(ledger: &Rc<ClaimLedger>) -> impl FnOnce() -> Option<CountedClaim> + '_ {
        move || {
            ledger.made.set(ledger.made.get() + 1);
            ledger.held.set(ledger.held.get() + 1);
            Some(CountedClaim(ledger.clone()))
        }
    }

    /// Spec (`take_plan_offer`): a Setup, Uninstall or Update answer submits
    /// the plan its render drew, and only while that showing is still open.
    /// An answer drawn for a plan since replaced, or for a showing since
    /// cancelled and reopened with the same plan, is stale: it submits
    /// nothing, never takes the page lock, and leaves the question now on
    /// screen up. A busy page refuses and keeps the question; a changed
    /// binding withdraws it without taking the lock; a genuine answer takes
    /// the showing and is accepted with its claim.
    ///
    /// Why: the answer spends the plan's one-use `probe_id` and carries out
    /// its steps on the host. Before these answers carried the showing they
    /// were drawn for, the confirm read whichever plan the row held at click
    /// time, so an answer for one plan could submit a later plan the user had
    /// not read. The claim counts pin `take_plan_offer`'s ordering: staleness
    /// and the binding are decided before `claim` runs, so a regression that
    /// claimed first and dropped the claim on a miss would fail here even
    /// though it leaves no claim held.
    #[farhelm_testtrace::test]
    fn a_plan_answer_submits_only_the_showing_it_was_drawn_for() {
        with_plan_slot(|mut slot| {
            let ledger = Rc::new(ClaimLedger::default());
            let mut first = flow_plan(ProvisioningOperation::Uninstall);
            first.probe_id = "plan-first".into();
            let mut second = first.clone();
            second.probe_id = "plan-second".into();
            second.confirmation = "a different plan".into();
            let binding = first.binding.clone();

            // A newer plan replaced the one the answer was drawn for.
            let drawn = slot.open(first.clone(), ());
            let replacing = slot.open(second.clone(), ());
            assert_eq!(
                take_plan_offer(&mut slot, &drawn, &binding, counted_claim(&ledger)),
                PlanConfirm::Stale
            );
            assert_eq!(ledger.made.get(), 0, "a stale answer never claims");
            assert!(slot.is_current(&replacing), "the newer plan stays up");

            // Cancelled, then the same plan shown again: the earlier
            // showing's answer still lands nowhere.
            assert!(slot.take(&replacing).is_some());
            let reopened = slot.open(second.clone(), ());
            assert_eq!(
                take_plan_offer(&mut slot, &replacing, &binding, counted_claim(&ledger)),
                PlanConfirm::Stale
            );
            assert_eq!(ledger.made.get(), 0, "a stale answer never claims");
            assert!(slot.is_current(&reopened), "the reopened plan stays up");

            // A busy page refuses and keeps the question.
            assert_eq!(
                take_plan_offer(&mut slot, &reopened, &binding, || None::<CountedClaim>),
                PlanConfirm::Refused
            );
            assert!(slot.is_current(&reopened));

            // A retargeted row withdraws the plan unsent.
            let mut retargeted = binding.clone();
            retargeted.destination = Some("elsewhere".into());
            assert_eq!(
                take_plan_offer(&mut slot, &reopened, &retargeted, counted_claim(&ledger)),
                PlanConfirm::BindingChanged
            );
            assert!(!slot.is_open());
            assert_eq!(ledger.made.get(), 0, "a withdrawn plan never claims");

            // A genuine answer takes its showing and holds the one claim.
            // The panel then submits `genuine.key()`, the plan that showing
            // drew.
            let genuine = slot.open(first.clone(), ());
            let PlanConfirm::Accepted(claim) =
                take_plan_offer(&mut slot, &genuine, &binding, counted_claim(&ledger))
            else {
                panic!("a current answer on an idle page is accepted");
            };
            assert_eq!(ledger.made.get(), 1, "the accepted answer claimed once");
            assert_eq!(ledger.held.get(), 1, "the accepted answer holds its claim");
            assert!(!slot.is_open(), "the showing is spent");
            drop(claim);

            // A second answer queued behind the accepted one finds nothing.
            assert_eq!(
                take_plan_offer(&mut slot, &genuine, &binding, counted_claim(&ledger)),
                PlanConfirm::Stale
            );
            assert_eq!(ledger.made.get(), 1, "the queued answer never claims");
            assert_eq!(ledger.held.get(), 0);
        });
    }

    /// Spec (the panel's remembered "don't ask in the future" setup answer):
    /// the remembered answer is bound to the token of the showing planning
    /// opened. If reconciliation clears that showing before the answer is
    /// applied (a run observed, a binding change, the row leaving local
    /// setup), the remembered answer is stale: it confirms neither that plan
    /// nor any plan opened after it, takes no page lock, and leaves the later
    /// plan up as an ordinary question.
    ///
    /// Why: the remembered answer used to be a bare flag that answered
    /// whatever plan was open when the auto-submit effect ran. A flag left
    /// set after reconciliation cleared its plan could then confirm a later
    /// plan (the defensive inline Update, say) that the remembered answer was
    /// never meant for, spending its `probe_id` without the user reading it.
    #[farhelm_testtrace::test]
    fn a_stale_remembered_answer_does_not_confirm_a_later_plan() {
        with_plan_slot(|mut slot| {
            let ledger = Rc::new(ClaimLedger::default());
            let mut setup = flow_plan(ProvisioningOperation::Add);
            setup.probe_id = "remembered-setup".into();
            let mut later = flow_plan(ProvisioningOperation::Update);
            later.probe_id = "later-update".into();
            let binding = setup.binding.clone();

            // Planning opens the setup plan and remembers its token; then
            // reconciliation retires it before the auto-submit runs, and a
            // later plan opens.
            let remembered = slot.open(setup, ());
            slot.clear();
            let later_showing = slot.open(later, ());

            assert_eq!(
                take_plan_offer(&mut slot, &remembered, &binding, counted_claim(&ledger)),
                PlanConfirm::Stale
            );
            assert_eq!(
                ledger.made.get(),
                0,
                "a stale remembered answer never claims"
            );
            assert!(
                slot.is_current(&later_showing),
                "the later plan stays up for the user to answer"
            );
        });
    }

    /// Spec (the panel's dismissal closure): the gate a Cancel, header
    /// dismiss or Escape consults, a `take` of the showing it was drawn for,
    /// refuses a showing since replaced and one a confirmation queued ahead
    /// of it already took, and closes nothing in either case. The panel's
    /// `dismiss` closure runs the rest of a dismissal (releasing the modal,
    /// moving focus to the row, clearing diagnostics, re-arming the local
    /// setup retry) only when this gate succeeds; those panel effects are not
    /// exercised here.
    ///
    /// Why: these dismissals used to close whatever plan was open, so one
    /// drawn for an earlier showing could take down the plan the user now
    /// sees, and one queued behind a confirmation could act after that
    /// confirmation had already taken the showing in `take_plan_offer`.
    #[farhelm_testtrace::test]
    fn a_plan_dismissal_acts_only_for_the_open_showing() {
        with_plan_slot(|mut slot| {
            let plan = flow_plan(ProvisioningOperation::Add);
            let earlier = slot.open(plan.clone(), ());
            let current = slot.open(plan.clone(), ());
            assert!(slot.take(&earlier).is_none());
            assert!(
                slot.is_current(&current),
                "a stale dismissal closes nothing"
            );

            assert!(slot.take(&current).is_some());
            assert!(!slot.is_open());

            // Queued behind a confirmation that already took the showing.
            let confirmed = slot.open(plan, ());
            let ledger = Rc::new(ClaimLedger::default());
            assert!(matches!(
                take_plan_offer(
                    &mut slot,
                    &confirmed,
                    &confirmed.key().binding,
                    counted_claim(&ledger)
                ),
                PlanConfirm::Accepted(..)
            ));
            assert!(slot.take(&confirmed).is_none());
        });
    }

    /// A confirmed plan closes before its POST. Only a definite remote
    /// refusal reopens the question; lost replies and local handoffs must
    /// remain with their existing row outcome paths.
    #[farhelm_testtrace::test]
    fn confirmed_submission_routing_separates_refusals_from_uncertainty() {
        let plan = |operation, kind| PendingPlan {
            operation,
            binding: HostBinding {
                id: 1,
                kind,
                destination: None,
                identity: None,
                remote_farhelm: None,
                remote_state_dir: None,
                incarnation: 0,
            },
            probe_id: "one-use".into(),
            confirmation: String::new(),
            steps: None,
            kept_state_dir: None,
        };
        let refused = SubmissionError::Refused("expired plan".into());
        let ambiguous = SubmissionError::Ambiguous("lost reply".into());
        for (operation, expected) in [
            (ProvisioningOperation::Add, HostRefusalAction::Setup),
            (
                ProvisioningOperation::Uninstall,
                HostRefusalAction::Uninstall,
            ),
        ] {
            let remote = plan(operation, HostKind::Ssh);
            let shown = confirmed_submission_refusal(&remote, &refused).unwrap();
            assert_eq!(shown.action, expected);
            assert_eq!(shown.reason, "expired plan");
            assert!(!shown.remove_instead);
            assert!(confirmed_submission_refusal(&remote, &ambiguous).is_none());
        }
        assert!(
            confirmed_submission_refusal(
                &plan(ProvisioningOperation::Add, HostKind::Local),
                &refused
            )
            .is_none()
        );
        assert!(
            confirmed_submission_refusal(
                &plan(ProvisioningOperation::Update, HostKind::Ssh),
                &refused
            )
            .is_none()
        );
    }

    /// Updates keep their own row busy without excluding Settings elsewhere.
    /// ADD/Uninstall preparation and pending plans still exclude other dialogs,
    /// including before they acquire the page operation lock. Reducing the raw
    /// lifecycle facts here pins that distinction without changing any offers.
    #[farhelm_testtrace::test]
    fn only_confirmation_preparation_and_pending_plans_exclude_other_settings() {
        for (confirmation_planning, has_pending_plan, update_owned, broad, modal) in [
            (false, false, false, false, false),
            (false, false, true, true, false),
            (true, false, false, true, true),
            (false, true, false, true, true),
            (true, true, true, true, true),
        ] {
            let menu = ProvisioningMenuState::offered(MenuFacts {
                confirmation_planning,
                has_pending_plan,
                update_owned,
                update_allowed: true,
                uninstall_allowed: true,
                ..MenuFacts::default()
            });
            assert_eq!(menu.planning, broad, "{menu:?}");
            assert_eq!(menu.confirmation_may_open, modal, "{menu:?}");
            assert_eq!(
                menu.update, !broad,
                "Update availability retains its broad guard"
            );
            assert_eq!(
                menu.uninstall, !broad,
                "Uninstall availability retains its broad guard"
            );
        }
    }

    /// A host running a newer farhelm than the helm is never offered Update
    /// from its row's menu, while an otherwise identical host is.
    ///
    /// Why it matters: the row offered Update on any remote host, and an
    /// Update on a newer host installs the helm's older build over it
    /// (SPEC.md: Update never downgrades a host). Specified: with every
    /// other fact allowing Update, it is offered unless the host is too
    /// new; the other offers are unaffected.
    #[farhelm_testtrace::test]
    fn a_too_new_host_is_not_offered_update() {
        let ready = MenuFacts {
            update_allowed: true,
            ..MenuFacts::default()
        };
        assert!(ProvisioningMenuState::offered(ready).update);
        let too_new = ProvisioningMenuState::offered(MenuFacts {
            too_new: true,
            ..ready
        });
        assert!(!too_new.update);
        assert_eq!(
            too_new,
            ProvisioningMenuState {
                update: false,
                ..ProvisioningMenuState::offered(ready)
            }
        );
        let failed_update = MenuFacts {
            failed_operation: Some(ProvisioningOperation::Update),
            ..ready
        };
        assert_eq!(
            ProvisioningMenuState::offered(failed_update).rerun,
            Some(ProvisioningOperation::Update)
        );
        assert_eq!(
            ProvisioningMenuState::offered(MenuFacts {
                too_new: true,
                ..failed_update
            })
            .rerun,
            None,
            "rerunning a failed Update is the same downgrade"
        );
    }

    /// The row menu offers uninstall only on a remote row with nothing in
    /// flight, and a failed uninstall is continued by that same item rather
    /// than offered as a rerun.
    ///
    /// Why it matters: SPEC.md makes uninstall unavailable while the host is
    /// busy with setup, an update or another uninstall, the way Update is;
    /// the local row is uninstalled with `farhelm uninstall`, never from the
    /// panel; and "choose uninstall again" is how a partial removal
    /// continues. Specified: offered when allowed and idle; withdrawn while a
    /// run, a plan or an update intent is in flight or when the kind does not
    /// allow it; still offered, with no rerun, after a failed uninstall.
    #[farhelm_testtrace::test]
    fn uninstall_is_offered_on_idle_remote_rows_and_continues_a_failed_one() {
        let ready = MenuFacts {
            update_allowed: true,
            uninstall_allowed: true,
            ..MenuFacts::default()
        };
        assert!(ProvisioningMenuState::offered(ready).uninstall);
        for busy in [
            MenuFacts {
                run_active: true,
                ..ready
            },
            MenuFacts {
                has_pending_plan: true,
                ..ready
            },
            MenuFacts {
                uninstall_allowed: false,
                ..ready
            },
        ] {
            assert!(!ProvisioningMenuState::offered(busy).uninstall, "{busy:?}");
        }
        let failed = ProvisioningMenuState::offered(MenuFacts {
            failed_operation: Some(ProvisioningOperation::Uninstall),
            ..ready
        });
        assert!(failed.uninstall);
        assert_eq!(failed.rerun, None);
    }

    /// A menu uninstall request plans like setup: it waits behind the page
    /// lock, ADD planning and a live update, and otherwise plans.
    ///
    /// Why it matters: uninstall shows a plan and submits only on the
    /// user's confirmation, so it must not start beside another plan of the
    /// same row or slip past the page lock that confirmation needs.
    /// Specified: refused while any of those blockers stands, planned once
    /// none does, and dropped as stale when the row changed since the click.
    #[farhelm_testtrace::test]
    fn an_uninstall_request_waits_like_setup_and_then_plans() {
        let host_id = 7;
        let request = ActionRequest {
            operation: ProvisioningOperation::Uninstall,
            binding: ssh_binding(host_id),
        };
        let current = ssh_binding(host_id);
        for (update_owned, add_planning, page_busy) in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
        ] {
            assert_eq!(
                decide_request(&request, &current, update_owned, add_planning, page_busy),
                RequestDecision::Refuse
            );
        }
        assert_eq!(
            decide_request(&request, &current, false, false, false),
            RequestDecision::Plan
        );
        let retargeted = HostBinding {
            destination: Some("elsewhere@example.invalid".to_string()),
            ..ssh_binding(host_id)
        };
        assert_eq!(
            decide_request(&request, &retargeted, false, false, false),
            RequestDecision::Stale
        );
    }

    fn ssh_binding(id: HostId) -> HostBinding {
        HostBinding {
            id,
            kind: HostKind::Ssh,
            destination: Some("e2e@example.invalid".to_string()),
            identity: Some("identity-1".to_string()),
            remote_farhelm: None,
            remote_state_dir: None,
            incarnation: 3,
        }
    }

    fn local_binding(id: HostId) -> HostBinding {
        HostBinding {
            id,
            kind: HostKind::Local,
            destination: None,
            identity: None,
            remote_farhelm: None,
            remote_state_dir: None,
            incarnation: 1,
        }
    }

    /// A menu command must survive the narrow race where another plan or
    /// page operation becomes busy before the mounted panel consumes it.
    /// The second pass proves consumption, rather than observation, is the
    /// boundary that removes the one-shot request.
    #[farhelm_testtrace::test]
    fn refused_menu_request_remains_queued_until_a_later_pass_accepts_it() {
        let host_id = 7;
        let request = ActionRequest {
            operation: ProvisioningOperation::Update,
            binding: ssh_binding(host_id),
        };
        let mut requests = HashMap::from([(host_id, request.clone())]);
        let mut attempts = 0;

        let refused = accepted_request(&requests, host_id, |seen| {
            attempts += 1;
            assert_eq!(seen, &request);
            false
        });
        assert_eq!(refused, None);
        assert_eq!(requests.get(&host_id), Some(&request));

        let accepted = accepted_request(&requests, host_id, |_| {
            attempts += 1;
            true
        });
        if accepted.is_some_and(|seen| requests.get(&host_id) == Some(&seen)) {
            requests.remove(&host_id);
        }
        assert!(!requests.contains_key(&host_id));
        assert_eq!(attempts, 2);
    }

    /// A click queued behind a retarget must not become an update of the
    /// retargeted row. The click-time binding is the evidence; any drift
    /// drops the request instead of planning against new facts.
    #[farhelm_testtrace::test]
    fn a_request_aimed_at_a_superseded_binding_is_stale() {
        let host_id = 7;
        let mut moved = ssh_binding(host_id);
        moved.destination = Some("moved@example.invalid".to_string());
        for operation in [ProvisioningOperation::Add, ProvisioningOperation::Update] {
            let request = ActionRequest {
                operation,
                binding: moved.clone(),
            };
            assert_eq!(
                decide_request(&request, &ssh_binding(host_id), false, false, false),
                RequestDecision::Stale,
                "{operation:?} against a retargeted row must not start",
            );
        }
    }

    /// Repeated update clicks coalesce into the accepted intent; they never
    /// queue a second update behind the first. ADD, which still confirms
    /// explicitly, refuses behind a live intent so the slower path never
    /// yanks the automatic one.
    #[farhelm_testtrace::test]
    fn duplicate_update_clicks_coalesce_while_add_waits_for_the_intent() {
        let host_id = 7;
        let binding = ssh_binding(host_id);
        let update = ActionRequest {
            operation: ProvisioningOperation::Update,
            binding: binding.clone(),
        };
        let add = ActionRequest {
            operation: ProvisioningOperation::Add,
            binding,
        };
        assert_eq!(
            decide_request(&update, &ssh_binding(host_id), true, false, false),
            RequestDecision::Coalesce,
        );
        assert_eq!(
            decide_request(&add, &ssh_binding(host_id), true, false, false),
            RequestDecision::Refuse,
        );
    }

    /// Update planning mutates nothing, so a held page lock never refuses
    /// it — the submission claim retries reactively instead. ADD planning
    /// still refuses behind the lock, and an update refuses behind ADD
    /// planning, whose completion it would otherwise race.
    #[farhelm_testtrace::test]
    fn update_planning_ignores_the_page_lock_but_not_add_planning() {
        let host_id = 7;
        let binding = ssh_binding(host_id);
        let update = ActionRequest {
            operation: ProvisioningOperation::Update,
            binding: binding.clone(),
        };
        let add = ActionRequest {
            operation: ProvisioningOperation::Add,
            binding,
        };
        assert_eq!(
            decide_request(&update, &ssh_binding(host_id), false, false, true),
            RequestDecision::Plan,
        );
        assert_eq!(
            decide_request(&add, &ssh_binding(host_id), false, false, true),
            RequestDecision::Refuse,
        );
        assert_eq!(
            decide_request(&update, &ssh_binding(host_id), false, true, false),
            RequestDecision::Refuse,
        );
    }

    /// The row-menu path skips only remote UPDATE. ADD's optional permanent
    /// answer is evaluated by the add-host dialog after its probe, and
    /// anything that is not an ssh row stays confirmed here.
    #[farhelm_testtrace::test]
    fn only_remote_updates_submit_without_confirmation() {
        let host_id = 7;
        assert!(automatic_update(
            ProvisioningOperation::Update,
            &ssh_binding(host_id)
        ));
        assert!(!automatic_update(
            ProvisioningOperation::Update,
            &local_binding(host_id)
        ));
        assert!(!automatic_update(
            ProvisioningOperation::Add,
            &ssh_binding(host_id)
        ));
        assert!(!automatic_update(
            ProvisioningOperation::Add,
            &local_binding(host_id)
        ));
    }

    /// An update normally changes its own connection incarnation mid-run,
    /// so incarnation churn — and learned install paths — must not break
    /// run continuity. A new id, kind, destination, or replacement of a
    /// known identity is a real target change; old run evidence must not
    /// follow it.
    #[farhelm_testtrace::test]
    fn run_continuity_survives_reconnects_but_not_retargets() {
        let host_id = 7;
        let before = ssh_binding(host_id);
        let mut reconnected = before.clone();
        reconnected.incarnation += 1;
        reconnected.remote_farhelm = Some("/home/e2e/.local/bin/farhelm".to_string());
        assert!(same_update_target(&before, &reconnected));

        let mut retargeted = before.clone();
        retargeted.destination = Some("moved@example.invalid".to_string());
        assert!(!same_update_target(&before, &retargeted));

        let mut reidentified = before.clone();
        reidentified.identity = Some("identity-2".to_string());
        assert!(!same_update_target(&before, &reidentified));
    }

    /// A protocol-skewed host can first report its identity on the update's
    /// reconnect. That permits continuity in one direction only: once the
    /// identity is learned and retained, losing or replacing it must detach
    /// the run. Exact equality for an unsubmitted plan stays stricter.
    #[farhelm_testtrace::test]
    fn run_continuity_allows_only_the_first_learned_identity() {
        let known = ssh_binding(7);
        let mut unknown = known.clone();
        unknown.identity = None;
        assert!(same_update_target(&unknown, &unknown));
        assert!(same_update_target(&unknown, &known));
        assert_ne!(unknown, known);
        assert!(same_update_target(&known, &known));
        assert!(!same_update_target(&known, &unknown));

        let mut replacement = known.clone();
        replacement.identity = Some("identity-2".to_string());
        assert!(!same_update_target(&known, &replacement));
    }

    /// Authoritative success is exact: the tracked nonempty run id, UPDATE
    /// on both sides, aggregate Completed. A 202 has no view at all; the
    /// idle view's Completed-with-no-run-id, a bare Connected row, an
    /// older run's success, and an ADD success each fail one conjunct.
    #[farhelm_testtrace::test]
    fn authoritative_success_names_the_exact_tracked_update_run() {
        fn view(
            run_id: Option<&str>,
            operation: Option<ProvisioningOperation>,
        ) -> ProvisioningView {
            ProvisioningView {
                run_id: run_id.map(str::to_string),
                operation,
                status: ProvisioningStatus::Completed,
                steps: Vec::new(),
                message: None,
            }
        }
        let tracked = TrackedRun {
            host: 7,
            run_id: "run-9".to_string(),
            operation: Some(ProvisioningOperation::Update),
            source: RunSource::Submitted { epoch: 2 },
            binding: ssh_binding(7),
            last_status: Some(ProvisioningStatus::Running),
            started_at: Instant::now(),
        };
        assert!(authoritative_update_success(
            &view(Some("run-9"), Some(ProvisioningOperation::Update)),
            &tracked,
        ));
        // An older run's success never settles the tracked pair.
        assert!(!authoritative_update_success(
            &view(Some("run-8"), Some(ProvisioningOperation::Update)),
            &tracked,
        ));
        // The idle view is Completed with no run id: not evidence.
        assert!(!authoritative_update_success(&view(None, None), &tracked,));
        // Neither is another operation's success for the same-named run.
        assert!(!authoritative_update_success(
            &view(Some("run-9"), Some(ProvisioningOperation::Add)),
            &tracked,
        ));
        // An empty tracked id can never be matched, so a malformed
        // acceptance must not be tracked in the first place.
        let empty = TrackedRun {
            run_id: String::new(),
            ..tracked.clone()
        };
        assert!(!authoritative_update_success(
            &view(Some("run-9"), Some(ProvisioningOperation::Update)),
            &empty,
        ));
        // A still-running view is progress, not success.
        let mut running = view(Some("run-9"), Some(ProvisioningOperation::Update));
        running.status = ProvisioningStatus::Running;
        assert!(!authoritative_update_success(&running, &tracked));
    }

    /// Retarget advice names the changed fact and the action the user can take.
    ///
    /// Destination wins when both fields change. Destinations stay raw until
    /// the peer renderer, so controls cannot be double-escaped, and missing
    /// identity remains the same phrase the host detail line uses.
    #[farhelm_testtrace::test]
    fn changed_host_advice_names_values_and_submission_boundary() {
        let before = ssh_binding(7);
        let moved = HostBinding {
            destination: Some("other\u{202e}@example.invalid".to_string()),
            identity: Some("identity-2".to_string()),
            ..before.clone()
        };
        for submitting in [false, true] {
            let text = changed_host_warning(&before, &moved, submitting);
            assert!(text.contains("ssh destination changed from ‹e2e@example.invalid› to ‹other\u{202e}@example.invalid›"));
            assert!(!text.contains("identity changed"));
            assert!(text.contains(if submitting {
                "while its update was being sent"
            } else {
                "while it was being updated"
            }));
            assert!(text.contains(if submitting {
                "can't tell whether the update started"
            } else {
                "can't tell how it ended or which install it reached"
            }));
            assert!(text.ends_with("update it again if it isn't current"));
        }
        let replaced = HostBinding {
            identity: Some("identity-2".to_string()),
            ..before.clone()
        };
        let text = changed_host_warning(&before, &replaced, false);
        assert!(text.contains("identity changed from ‹identity-1› to ‹identity-2›"));
        assert!(text.contains("a different Farhelm install now answers"));

        let missing = HostBinding {
            identity: None,
            ..before.clone()
        };
        let text = changed_host_warning(&before, &missing, true);
        assert!(text.contains("identity changed from ‹identity-1› to ‹no identity reported›"));
        assert!(text.contains("can no longer be identified"));
        assert!(!text.contains("a different Farhelm install now answers"));
        for changed in [local_binding(7), ssh_binding(8)] {
            let text = changed_host_warning(&before, &changed, false);
            assert!(text.starts_with("this host's entry changed"));
            assert!(!text.contains("identity-1"));
            assert!(!text.contains("e2e@example.invalid"));
        }
    }

    /// The user reads the two identities in this warning to judge whether a
    /// different install now answers, so two different identities must never
    /// render alike. The warning reaches the page as one peer run, and the
    /// peer renderer alone shows an actual U+202E and the literal text
    /// `<U+202E>` identically; spelling each identity first keeps them apart,
    /// while an ordinary identity still reads as itself.
    #[farhelm_testtrace::test]
    fn changed_host_identities_never_render_alike() {
        let before = ssh_binding(7);
        let rendered = |identity: &str| {
            let now = HostBinding {
                identity: Some(identity.to_string()),
                ..before.clone()
            };
            crate::peer::detail_text(&[DetailPart::peer(changed_host_warning(
                &before, &now, false,
            ))])
        };
        let (control, literal) = ("x\u{202e}y", "x<U+202E>y");
        assert_eq!(
            display_peer(control),
            display_peer(literal),
            "premise: the peer renderer alone conflates the two"
        );
        assert_ne!(rendered(control), rendered(literal));
        assert!(rendered(control).contains("to ‹x<U+202E>y›"));
        assert!(rendered(literal).contains("to ‹x<U+003C>U+202E>y›"));
        assert!(rendered("identity-2").contains("to ‹identity-2›"));
    }

    /// A later host change must not turn unresolved update evidence into clearable advice.
    ///
    /// Both constructors share this priority rule because a held acceptance
    /// and a followed run can each coexist with an older sticky warning.
    #[farhelm_testtrace::test]
    fn host_change_advice_preserves_existing_submission_uncertainty() {
        let before = ssh_binding(7);
        let after = HostBinding {
            destination: Some("moved@example.invalid".to_string()),
            ..before.clone()
        };
        let unresolved = UpdateDiagnostic {
            epoch: 3,
            sticky: true,
            host_changed: false,
            text: "earlier update ended unseen".to_string(),
        };
        for submitting in [false, true] {
            assert_eq!(
                changed_host_diagnostic(Some(&unresolved), 4, &before, &after, submitting),
                None
            );
            let advice = changed_host_diagnostic(None, 4, &before, &after, submitting).unwrap();
            assert!(advice.host_changed && advice.sticky);
            assert_eq!(advice.epoch, 4);
            assert!(
                changed_host_diagnostic(Some(&advice), 5, &before, &after, submitting).is_some()
            );
            let transient = UpdateDiagnostic {
                sticky: false,
                ..unresolved.clone()
            };
            assert!(
                changed_host_diagnostic(Some(&transient), 5, &before, &after, submitting).is_some()
            );
        }
    }

    /// A stale browser must compare against the helm, and silence proves no version.
    ///
    /// These inputs are injected rather than changing global signals or the
    /// test process environment. The cases pin the existing skew evidence's
    /// meaning, including its latched reported build.
    #[farhelm_testtrace::test]
    fn helm_build_evidence_distinguishes_reported_matching_and_unknown() {
        let reported = crate::skew::Skew::Reported("helm-other-build".to_string());
        let silent = crate::skew::Skew::Silent;
        assert_eq!(
            known_helm_build(Some(&reported), true),
            Some("helm-other-build")
        );
        assert_eq!(
            known_helm_build(Some(&reported), false),
            Some("helm-other-build")
        );
        assert_eq!(
            known_helm_build(None, true),
            Some(crate::skew::CLIENT_BUILD)
        );
        assert_eq!(known_helm_build(None, false), None);
        assert_eq!(known_helm_build(Some(&silent), false), None);
        assert_eq!(known_helm_build(Some(&silent), true), None);
    }

    /// Connected at an exactly matching build clears only change-of-host advice.
    ///
    /// Neither older/newer hints nor a healthy connection can settle a lost
    /// reply. Exact build comparison also matters for non-semver development
    /// stamps and a browser bundle that differs from its helm.
    #[farhelm_testtrace::test]
    fn current_connection_resolves_only_a_change_host_warning() {
        let warning = UpdateDiagnostic {
            epoch: 3,
            sticky: true,
            host_changed: true,
            text: "changed host".to_string(),
        };
        let connected = crate::HostPhase::Connected {
            identity: Some("identity-2".into()),
            build_version: "helm-build".into(),
            old_version: false,
            newer_version: false,
            refresh: crate::RefreshHealth::Ok { sessions: 0 },
        };
        assert!(change_warning_resolved(
            &warning,
            &connected,
            Some("helm-build")
        ));
        for build in [
            None,
            Some("older-build"),
            Some("newer-build"),
            Some("helm-build "),
        ] {
            assert!(!change_warning_resolved(&warning, &connected, build));
        }
        let disconnected = crate::HostPhase::Unreachable {
            cause: "transport-failure".to_string(),
            last_error: "not connected".into(),
        };
        assert!(!change_warning_resolved(
            &warning,
            &disconnected,
            Some("helm-build")
        ));
        let other = UpdateDiagnostic {
            host_changed: false,
            ..warning
        };
        assert!(!change_warning_resolved(
            &other,
            &connected,
            Some("helm-build")
        ));
    }

    /// A live update stays folded, while failures and unresolved outcomes keep the row open.
    ///
    /// This truth table protects the distinction between compact progress and
    /// diagnostic evidence; losing it either hides a failure or restores the
    /// unwanted expansion for every ordinary update.
    #[farhelm_testtrace::test]
    fn automatic_disclosure_opens_only_for_failure_or_diagnostic() {
        let binding = ssh_binding(7);
        let running = TrackedRun {
            host: 7,
            run_id: "run-9".to_string(),
            operation: Some(ProvisioningOperation::Update),
            source: RunSource::Observed,
            binding: binding.clone(),
            last_status: Some(ProvisioningStatus::Running),
            started_at: Instant::now(),
        };
        let failed = TrackedRun {
            last_status: Some(ProvisioningStatus::Failed),
            ..running.clone()
        };
        let add_failed = TrackedRun {
            operation: Some(ProvisioningOperation::Add),
            ..failed.clone()
        };
        let diagnostic = UpdateDiagnostic {
            epoch: 1,
            sticky: true,
            host_changed: false,
            text: "outcome unknown".to_string(),
        };

        assert!(!update_auto_disclosed(None, None, None, None));
        assert!(!update_auto_disclosed(Some(&running), None, None, None));
        assert!(update_auto_disclosed(
            Some(&running),
            None,
            None,
            Some("read failed")
        ));
        assert!(update_auto_disclosed(Some(&failed), None, None, None));
        assert!(update_auto_disclosed(None, Some(&diagnostic), None, None));
        assert!(update_auto_disclosed(None, None, Some(&diagnostic), None));
        assert!(!update_auto_disclosed(
            Some(&add_failed),
            None,
            None,
            Some("read failed")
        ));
    }

    /// A fold is one run's choice, while failure restores the steps needed to diagnose it.
    ///
    /// Setup and uninstall use the same host-scoped progress feed as Update.
    /// This decision must key on its opaque run id: repeated snapshots cannot
    /// reopen a folded run, and a later run must not inherit that fold.
    #[farhelm_testtrace::test]
    fn confirmed_run_expansion_is_one_off_per_run_id() {
        let mut view = ProvisioningView {
            run_id: Some("run-a".to_string()),
            operation: Some(ProvisioningOperation::Add),
            status: ProvisioningStatus::Running,
            steps: vec![],
            message: None,
        };
        let first = confirmed_run_disclosure(None, Some(&view)).expect("running setup opens");
        assert!(!first.folded);
        let folded = ConfirmedRunDisclosure {
            folded: true,
            ..first
        };
        assert_eq!(
            confirmed_run_disclosure(Some(&folded), Some(&view)),
            Some(folded.clone())
        );
        view.status = ProvisioningStatus::Failed;
        let failed_folded =
            confirmed_run_disclosure(Some(&folded), Some(&view)).expect("failure reopens");
        assert!(!failed_folded.folded);
        assert!(failed_folded.failed);
        view.status = ProvisioningStatus::Running;

        view.run_id = Some("run-b".to_string());
        let next = confirmed_run_disclosure(Some(&folded), Some(&view)).expect("new run opens");
        assert!(!next.folded);
        assert_eq!(next.run_id, "run-b");

        view.status = ProvisioningStatus::Failed;
        let failure = confirmed_run_disclosure(Some(&folded), Some(&view)).expect("failure opens");
        assert!(failure.failed);
        assert!(!failure.folded);
        view.status = ProvisioningStatus::Completed;
        assert_eq!(confirmed_run_disclosure(Some(&failure), Some(&view)), None);

        view.operation = Some(ProvisioningOperation::Uninstall);
        view.status = ProvisioningStatus::Running;
        assert!(confirmed_run_disclosure(None, Some(&view)).is_some());
        view.operation = Some(ProvisioningOperation::Update);
        assert_eq!(confirmed_run_disclosure(None, Some(&view)), None);
    }

    /// Inline progress counts terminal steps and names only a currently running step.
    ///
    /// The row cannot show the full trace while folded, so this summary must
    /// preserve executor order, retain warning reasons for its popup, and
    /// reject stale snapshots that name another run.
    #[farhelm_testtrace::test]
    fn update_progress_summary_counts_steps_and_requires_the_tracked_live_run() {
        let started_at = Instant::now();
        let tracked = TrackedRun {
            host: 7,
            run_id: "run-9".to_string(),
            operation: Some(ProvisioningOperation::Update),
            source: RunSource::Observed,
            binding: ssh_binding(7),
            last_status: Some(ProvisioningStatus::Running),
            started_at,
        };
        let view = ProvisioningView {
            run_id: Some("run-9".to_string()),
            operation: Some(ProvisioningOperation::Update),
            status: ProvisioningStatus::Running,
            steps: vec![
                crate::api::ProvisioningStep {
                    step: "check-host".to_string(),
                    status: "completed".to_string(),
                    message: None,
                    ..Default::default()
                },
                crate::api::ProvisioningStep {
                    step: "upload-farhelm".to_string(),
                    name: Some("upload the farhelm binary".into()),
                    status: "running".to_string(),
                    message: None,
                    ..Default::default()
                },
                crate::api::ProvisioningStep {
                    step: "install-supervisor".to_string(),
                    status: "pending".to_string(),
                    message: None,
                    ..Default::default()
                },
                crate::api::ProvisioningStep {
                    step: "optional-step".to_string(),
                    status: "skipped".to_string(),
                    message: None,
                    ..Default::default()
                },
                crate::api::ProvisioningStep {
                    step: "degraded-step".to_string(),
                    status: "degraded".to_string(),
                    message: Some("starts at login instead".into()),
                    ..Default::default()
                },
                crate::api::ProvisioningStep {
                    step: "failed-step".to_string(),
                    status: "failed".to_string(),
                    message: None,
                    ..Default::default()
                },
                crate::api::ProvisioningStep {
                    step: "future-step".to_string(),
                    status: "new-status".to_string(),
                    message: None,
                    ..Default::default()
                },
            ],
            message: None,
        };
        let summary = update_progress_summary(&view, &tracked).expect("live run has progress");
        assert_eq!(
            summary.current_step.as_deref(),
            Some("upload the farhelm binary")
        );
        assert_eq!((summary.done, summary.total), (4, 7));
        assert_eq!(summary.started_at, started_at);
        // The hover popup's list: every step in executor order, statuses as
        // supplied, and exactly the step `current_step` names marked current.
        assert_eq!(
            summary
                .steps
                .iter()
                .map(|step| step.id.as_str())
                .collect::<Vec<_>>(),
            view.steps
                .iter()
                .map(|step| step.step.as_str())
                .collect::<Vec<_>>(),
        );
        assert_eq!(summary.steps[1].name, "upload the farhelm binary");
        assert_eq!(
            summary.steps[4].message.as_deref(),
            Some("starts at login instead")
        );
        assert_eq!(progress_step_status("skipped"), "unchanged");
        assert_eq!(progress_step_status("degraded"), "warning");
        assert_eq!(progress_step_status("new-status"), "new-status");
        assert!(
            summary
                .steps
                .iter()
                .zip(&view.steps)
                .all(|(line, step)| line.status == step.status)
        );
        assert_eq!(
            summary
                .steps
                .iter()
                .filter(|step| step.current)
                .map(|step| step.name.as_str())
                .collect::<Vec<_>>(),
            vec!["upload the farhelm binary"],
        );

        let mut other_run = view.clone();
        other_run.run_id = Some("run-8".to_string());
        assert_eq!(update_progress_summary(&other_run, &tracked), None);
        let mut terminal = view;
        terminal.status = ProvisioningStatus::Failed;
        assert_eq!(update_progress_summary(&terminal, &tracked), None);
    }

    /// Confirmed setup and uninstall keep the row's status current without a dialog.
    ///
    /// Once uninstall stops the supervisor, the ordinary connection status
    /// would read unreachable while removal still runs. Both operations use
    /// a run-id clock and helm-authored step names; stale or finished views
    /// cannot claim that the work is still live.
    #[farhelm_testtrace::test]
    fn a_running_uninstall_shows_inline_like_an_update() {
        let started_at = Instant::now();
        let clock = ("run-u".to_string(), started_at);
        let view = ProvisioningView {
            run_id: Some("run-u".to_string()),
            operation: Some(ProvisioningOperation::Uninstall),
            status: ProvisioningStatus::Running,
            steps: vec![
                crate::api::ProvisioningStep {
                    step: "disable-supervisor".to_string(),
                    status: "completed".to_string(),
                    message: None,
                    ..Default::default()
                },
                crate::api::ProvisioningStep {
                    step: "remove-unit".to_string(),
                    status: "running".to_string(),
                    message: None,
                    ..Default::default()
                },
            ],
            message: None,
        };
        let Some(HostUpdateProgress::Running(summary)) =
            derive_row_update_progress(None, None, Some(&view), Some(&clock))
        else {
            panic!("a running uninstall must show inline progress");
        };
        assert_eq!(summary.operation, ProvisioningOperation::Uninstall);
        assert_eq!(summary.verb(), "uninstalling");
        assert_eq!(summary.current_step.as_deref(), Some("remove-unit"));
        assert_eq!((summary.done, summary.total), (1, 2));
        assert_eq!(summary.started_at, started_at);

        let setup = ProvisioningView {
            operation: Some(ProvisioningOperation::Add),
            ..view.clone()
        };
        let Some(HostUpdateProgress::Running(setup_summary)) =
            derive_row_update_progress(None, None, Some(&setup), Some(&clock))
        else {
            panic!("a running setup must show inline progress");
        };
        assert_eq!(setup_summary.verb(), "setting up");

        assert_eq!(
            derive_row_update_progress(None, None, Some(&view), None),
            None
        );
        let other_clock = ("run-other".to_string(), started_at);
        assert_eq!(
            derive_row_update_progress(None, None, Some(&view), Some(&other_clock)),
            None
        );
        let finished = ProvisioningView {
            status: ProvisioningStatus::Failed,
            ..view.clone()
        };
        assert_eq!(
            derive_row_update_progress(None, None, Some(&finished), Some(&clock)),
            None
        );
    }

    /// The row's inline status covers the whole update lifecycle and names each pending stage.
    ///
    /// Browser tests hold planning, the claim wait, and submission at exact
    /// boundaries while the row stays folded, and they read the published
    /// stage to know which boundary the UI reached. The status must also
    /// stand from the click until a terminal observation — never dropping to
    /// the ordinary label mid-update — and must vanish for terminal, ADD, or
    /// absent runs so the ordinary label (and the collapsed trace for ADD)
    /// returns.
    #[farhelm_testtrace::test]
    fn row_update_progress_names_each_stage_and_clears_after_the_run() {
        let intent = |phase| UpdateIntent {
            epoch: 1,
            binding: ssh_binding(7),
            phase,
            plan: None,
        };
        let pending = |phase| Some(HostUpdateProgress::Pending(phase));
        assert_eq!(
            derive_row_update_progress(Some(&intent(IntentPhase::Planning)), None, None, None),
            pending(UpdatePendingPhase::Planning)
        );
        assert_eq!(
            derive_row_update_progress(
                Some(&intent(IntentPhase::WaitingForClaim)),
                None,
                None,
                None
            ),
            pending(UpdatePendingPhase::WaitingForClaim)
        );

        let started_at = Instant::now();
        let running_run = TrackedRun {
            host: 7,
            run_id: "run-9".to_string(),
            operation: Some(ProvisioningOperation::Update),
            source: RunSource::Submitted { epoch: 1 },
            binding: ssh_binding(7),
            last_status: Some(ProvisioningStatus::Running),
            started_at,
        };
        let running_view = ProvisioningView {
            run_id: Some("run-9".to_string()),
            operation: Some(ProvisioningOperation::Update),
            status: ProvisioningStatus::Running,
            steps: vec![crate::api::ProvisioningStep {
                step: "create-directories".to_string(),
                status: "running".to_string(),
                message: None,
                ..Default::default()
            }],
            message: None,
        };
        // A live intent outranks whatever run and view are also present.
        assert_eq!(
            derive_row_update_progress(
                Some(&intent(IntentPhase::Submitting)),
                Some(&running_run),
                Some(&running_view),
                None
            ),
            pending(UpdatePendingPhase::Submitting)
        );

        // Accepted but not yet read, or displaying some other run's view.
        let accepted = TrackedRun {
            last_status: None,
            ..running_run.clone()
        };
        assert_eq!(
            derive_row_update_progress(None, Some(&accepted), None, None),
            pending(UpdatePendingPhase::AwaitingProgress)
        );
        let mut other_view = running_view.clone();
        other_view.run_id = Some("run-8".to_string());
        assert_eq!(
            derive_row_update_progress(None, Some(&running_run), Some(&other_view), None),
            pending(UpdatePendingPhase::AwaitingProgress)
        );

        let Some(HostUpdateProgress::Running(summary)) =
            derive_row_update_progress(None, Some(&running_run), Some(&running_view), None)
        else {
            panic!("the tracked run's own running view shows full progress");
        };
        assert_eq!(summary.current_step.as_deref(), Some("create-directories"));
        assert_eq!((summary.done, summary.total), (0, 1));

        for status in [ProvisioningStatus::Completed, ProvisioningStatus::Failed] {
            let terminal = TrackedRun {
                last_status: Some(status),
                ..running_run.clone()
            };
            assert_eq!(
                derive_row_update_progress(None, Some(&terminal), None, None),
                None
            );
        }
        let add_run = TrackedRun {
            operation: Some(ProvisioningOperation::Add),
            ..running_run
        };
        assert_eq!(
            derive_row_update_progress(None, Some(&add_run), None, None),
            None
        );
        assert_eq!(
            derive_row_update_progress(None, None, Some(&running_view), None),
            None
        );
    }

    /// Ownership outlives the intent: admission, the menu, and busy paint all
    /// derive from the intent plus the retained tracked run, so an unrelated
    /// Completed view can neither re-offer Update nor release the row while
    /// the accepted run is still retained. A terminal observation ends
    /// ownership and deliberate retry becomes available again. Unlike
    /// disclosure, ownership does not filter by operation: any live run
    /// excludes overlap.
    #[farhelm_testtrace::test]
    fn update_ownership_survives_acceptance_until_a_terminal_state() {
        let binding = ssh_binding(7);
        let intent = UpdateIntent {
            epoch: 1,
            binding: binding.clone(),
            phase: IntentPhase::Submitting,
            plan: None,
        };
        fn tracked(
            operation: Option<ProvisioningOperation>,
            last_status: Option<ProvisioningStatus>,
        ) -> TrackedRun {
            TrackedRun {
                host: 7,
                run_id: "run-9".to_string(),
                operation,
                source: RunSource::Submitted { epoch: 1 },
                binding: ssh_binding(7),
                last_status,
                started_at: Instant::now(),
            }
        }

        // Nothing retained and no intent: the row is free.
        assert!(!retained_run_live(None));
        assert!(!update_owned(None, None));

        // A live intent owns the row whatever the tracked state.
        assert!(update_owned(Some(&intent), None));
        let failed = tracked(
            Some(ProvisioningOperation::Update),
            Some(ProvisioningStatus::Failed),
        );
        assert!(update_owned(Some(&intent), Some(&failed)));

        // After acceptance the intent is gone but the retained run still
        // owns the row: the gap before the first read, and a Running
        // observation, are both nonterminal.
        let gap = tracked(Some(ProvisioningOperation::Update), None);
        assert!(retained_run_live(Some(&gap)));
        assert!(update_owned(None, Some(&gap)));
        let running = tracked(
            Some(ProvisioningOperation::Update),
            Some(ProvisioningStatus::Running),
        );
        assert!(update_owned(None, Some(&running)));

        // Any live run excludes overlap, whatever its operation.
        let add_running = tracked(
            Some(ProvisioningOperation::Add),
            Some(ProvisioningStatus::Running),
        );
        assert!(update_owned(None, Some(&add_running)));

        // A terminal observation ends ownership: failure and completion both
        // free the row for deliberate retry.
        let completed = tracked(
            Some(ProvisioningOperation::Update),
            Some(ProvisioningStatus::Completed),
        );
        assert!(!retained_run_live(Some(&completed)));
        assert!(!update_owned(None, Some(&completed)));
        assert!(!update_owned(None, Some(&failed)));
    }

    /// The words painted in a run header must cover every rendered state and
    /// operation. Both are also exposed as stable data attributes, so a
    /// rename here is a contract change rather than copy editing. The valid
    /// idle view has no header and therefore does not belong in this test.
    #[farhelm_testtrace::test]
    fn run_labels_cover_the_wire_vocabulary() {
        assert_eq!(status_label(ProvisioningStatus::Running), "running");
        assert_eq!(status_label(ProvisioningStatus::Completed), "completed");
        assert_eq!(status_label(ProvisioningStatus::Failed), "failed");
        assert_eq!(operation_label(Some(ProvisioningOperation::Add)), "setup");
        assert_eq!(
            operation_label(Some(ProvisioningOperation::Update)),
            "update"
        );
    }

    /// A competing run's non-sticky explanation must not erase a sticky
    /// submission uncertainty: the earlier request may have committed while
    /// this attempt provably submitted nothing, so the caller installs
    /// nothing and the held epoch and text survive. Without a sticky
    /// warning, the ordinary explanation installs as before.
    #[farhelm_testtrace::test]
    fn competing_run_explanation_yields_to_a_sticky_warning() {
        let sticky = UpdateDiagnostic {
            epoch: 3,
            sticky: true,
            host_changed: false,
            text: "the helm accepted the update, but its run identity could not be read"
                .to_string(),
        };
        assert_eq!(
            competing_run_warning(Some(&sticky), 4, "another run started".to_string()),
            None,
        );

        let plain = UpdateDiagnostic {
            epoch: 2,
            sticky: false,
            host_changed: false,
            text: "an earlier explanation".to_string(),
        };
        assert_eq!(
            competing_run_warning(Some(&plain), 4, "another run started".to_string()),
            Some(UpdateDiagnostic {
                epoch: 4,
                sticky: false,
                host_changed: false,
                text: "another run started".to_string(),
            }),
        );
        assert_eq!(
            competing_run_warning(None, 4, "another run started".to_string()),
            Some(UpdateDiagnostic {
                epoch: 4,
                sticky: false,
                host_changed: false,
                text: "another run started".to_string(),
            }),
        );
    }
}

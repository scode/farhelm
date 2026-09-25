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

use std::collections::{HashMap, HashSet};
use web_time::Instant;

use dioxus::prelude::*;

use crate::api::{
    ProbeResponse, ProvisioningAccepted, ProvisioningOperation, ProvisioningStatus,
    ProvisioningSubmission, ProvisioningView, SubmissionError, fetch_provisioning,
    plan_host_update, probe_local_host, probe_ssh_host, provision_host, update_host,
};
use crate::feed::{fallback_polls_now, fallback_sleep, use_feed_reader};
use crate::ops::{OpGuard, OpLock, ReadGate};
use crate::peer::{DetailPart, PeerBlock, PeerLine};
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
    /// A plan is in flight: ADD planning or a displayed offer, a live
    /// automatic-update intent in any phase, or a retained accepted/observed
    /// run that has not reached a terminal state. While set, the menu offers
    /// no new provisioning command and disables the in-flight one.
    pub(crate) planning: bool,
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
        ProvisioningOperation::Add if update_owned || add_planning || page_busy => {
            RequestDecision::Refuse
        }
        // A second ADD supersedes a merely displayed offer, as before: the
        // older plan is dropped unsent and planning starts over.
        ProvisioningOperation::Add => RequestDecision::Plan,
    }
}

/// Whether this operation on this binding submits without confirmation.
///
/// Remote UPDATE only. ADD keeps its confirmation in every shape (initial
/// setup, automatic local setup, rerun), and anything that is not an ssh row
/// confirms too — this gate must not be widened to the menu's broader
/// `update_allowed`, which also offers Update on local rows the backend's
/// `plan_update()` refuses.
fn automatic_update(operation: ProvisioningOperation, binding: &HostBinding) -> bool {
    operation == ProvisioningOperation::Update && binding.kind == HostKind::Ssh
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
            destination: host.destination.clone(),
            identity: host.identity.clone(),
            remote_farhelm: host.remote_farhelm.clone(),
            remote_state_dir: host.remote_state_dir.clone(),
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
/// install paths do not break continuity. Anything else is a real target
/// change, and old run evidence must not be reinterpreted as the new
/// target's history.
fn same_update_target(before: &HostBinding, now: &HostBinding) -> bool {
    before.id == now.id
        && before.kind == now.kind
        && before.destination == now.destination
        && before.identity == now.identity
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

/// The small per-host summary published from the provisioning panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpdateProgressSummary {
    /// The exact tracked run id, so replacing a run also replaces its clock.
    pub(crate) run_id: String,
    /// First executor-ordered `running` step, absent when the view names none.
    pub(crate) current_step: Option<String>,
    /// Steps in a terminal state: completed, skipped, degraded, or failed.
    pub(crate) done: usize,
    /// All executor steps in the snapshot, including pending work.
    pub(crate) total: usize,
    /// Client-local monotonic start; never a helm timestamp or persisted value.
    pub(crate) started_at: Instant,
}

/// One unresolved update diagnostic, kept apart from observed progress.
///
/// An unrelated completed run, an idle view, or a successful GET never
/// clears these; only a correlated newer success, an explicit superseding
/// attempt, or row removal does.
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
    text: String,
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
/// Only update failures, unresolved diagnostics, or a failed progress read during a live UPDATE open the row.
/// Running intent and progress stay inline; ADD confirmation and its global
/// reveal are unchanged, and a merely observed ADD run is trace enough.
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
    let current_step = view
        .steps
        .iter()
        .find(|step| step.status == "running")
        .map(|step| step.step.clone());

    Some(UpdateProgressSummary {
        run_id: tracked.run_id.clone(),
        current_step,
        done,
        total: view.steps.len(),
        started_at: tracked.started_at,
    })
}

/// The compact update status this row publishes to its host row, if any.
///
/// A live automatic-update intent always wins and names its own phase, so
/// the click shows `updating…` from the moment it is accepted and never
/// falls back to the ordinary label while the POST is outstanding. After the
/// intent ends, only a live tracked UPDATE run keeps a status: full progress
/// when the displayed view is that exact run running, otherwise the
/// awaiting-progress stage. Terminal runs, ADD runs, and rows with nothing
/// tracked publish nothing, which is what returns the ordinary label.
///
/// The same answer also decides whether the collapsed trace line is drawn:
/// while the row carries this inline status, a second line saying the update
/// is running would only repeat it.
fn derive_row_update_progress(
    intent: Option<&UpdateIntent>,
    tracked: Option<&TrackedRun>,
    view: Option<&ProvisioningView>,
) -> Option<HostUpdateProgress> {
    if let Some(live) = intent {
        return Some(HostUpdateProgress::Pending(match live.phase {
            IntentPhase::Planning => UpdatePendingPhase::Planning,
            IntentPhase::WaitingForClaim => UpdatePendingPhase::WaitingForClaim,
            IntentPhase::Submitting => UpdatePendingPhase::Submitting,
        }));
    }
    let run = tracked.filter(|run| {
        run.operation == Some(ProvisioningOperation::Update) && retained_run_live(Some(run))
    })?;
    Some(
        view.and_then(|view| update_progress_summary(view, run))
            .map_or(
                HostUpdateProgress::Pending(UpdatePendingPhase::AwaitingProgress),
                HostUpdateProgress::Running,
            ),
    )
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
#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingPlan {
    operation: ProvisioningOperation,
    binding: HostBinding,
    probe_id: String,
    confirmation: String,
}

/// What preparing a run established without yet mutating the target.
enum Preparation {
    Plan(PendingPlan),
    /// ADD found a supervisor and registered it as-is, so there is no run to
    /// confirm or follow.
    Discovered,
    /// The transport worked, but this target stays on the manual path.
    Manual(String),
    /// The probe may have registered a supervisor, but its successful reply
    /// did not say which outcome this client should render.
    Unvalidated(String),
}

/// Render the exact confirmation text returned by the helm.
///
/// This component deliberately does not walk `ProvisioningPlan.actions`.
/// The server's string was rendered from the same value execution consumes,
/// including paths, unit names, and the conditional linger promise. A second
/// renderer here would be a second list of promises that could drift.
#[component]
pub(crate) fn PlanConfirmation(
    confirmation: String,
    busy: bool,
    confirm_label: &'static str,
    on_confirm: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "provisioning-confirmation",
            PeerBlock { class: "provisioning-plan", text: confirmation }
            div { class: "provisioning-confirm-actions",
                button {
                    r#type: "button",
                    class: "btn btn-primary provisioning-confirm",
                    disabled: busy,
                    onclick: move |_| on_confirm.call(()),
                    "{confirm_label}"
                }
                button {
                    r#type: "button",
                    class: "btn btn-neutral provisioning-cancel",
                    disabled: busy,
                    onclick: move |_| on_cancel.call(()),
                    "cancel"
                }
            }
        }
    }
}

/// Prepare a fresh plan for the requested idempotent operation.
async fn prepare(
    base: &str,
    host: &Host,
    operation: ProvisioningOperation,
) -> Result<Preparation, String> {
    let binding = HostBinding::from(host);
    match operation {
        ProvisioningOperation::Update => {
            let planned = plan_host_update(base, host.id).await?;
            Ok(Preparation::Plan(PendingPlan {
                operation,
                binding,
                probe_id: planned.probe_id,
                confirmation: planned.confirmation,
            }))
        }
        ProvisioningOperation::Add => {
            let probed = match host.kind {
                HostKind::Local => probe_local_host(base).await?,
                HostKind::Ssh => {
                    let destination = host
                        .destination
                        .as_deref()
                        .ok_or_else(|| "this ssh host has no destination to probe".to_string())?;
                    probe_ssh_host(
                        base,
                        destination,
                        host.remote_farhelm.as_deref().unwrap_or_default(),
                        host.remote_state_dir.as_deref().unwrap_or_default(),
                    )
                    .await?
                }
                HostKind::Unrecognized => {
                    return Err(
                        "this build does not recognize the host kind, so it cannot provision it"
                            .to_string(),
                    );
                }
            };
            Ok(match probed {
                ProbeResponse::Discovered => Preparation::Discovered,
                ProbeResponse::Provisionable {
                    probe_id,
                    confirmation,
                } => Preparation::Plan(PendingPlan {
                    operation,
                    binding,
                    probe_id,
                    confirmation,
                }),
                ProbeResponse::Manual { reason } => Preparation::Manual(reason),
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

/// Submit one binding-checked plan exactly once, then reconcile its outcome.
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
fn submit_plan<R>(submission: Submission<R>, plan: PendingPlan)
where
    R: Fn(Trigger) + Clone + 'static,
{
    let Submission {
        base,
        host_id,
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
        reread,
    } = submission;
    spawn(async move {
        let result = match plan.operation {
            ProvisioningOperation::Add => provision_host(&base, &plan.probe_id).await,
            ProvisioningOperation::Update => update_host(&base, host_id, &plan.probe_id).await,
        };
        // Progress reads and registry reconciliation are ordinary reads;
        // release the page token as soon as submission has an outcome.
        drop(claim);
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
                                        text,
                                    }));
                                }
                            }
                            SubmissionError::Ambiguous(error) => {
                                update_error.set(Some(UpdateDiagnostic {
                                    epoch,
                                    sticky: true,
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
                        if plan.binding.kind == HostKind::Local
                            && plan.operation == ProvisioningOperation::Add
                        {
                            local_auto_retry.set(true);
                        }
                        action_error.set(Some(error.into_text()));
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
            if let Err(error) = fetch_provisioning(base, accepted.host_id).await {
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
    if !same_update_target(&plan.binding, &current) {
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
        update_warning.set(Some(UpdateDiagnostic {
            epoch,
            sticky: true,
            text: "the host changed while the update was submitting, so its outcome is unknown; \
                 this row claims nothing from that attempt"
                .to_string(),
        }));
        release_outstanding(outstanding, epoch);
        reread(Trigger::Explicit);
        on_changed.call(());
        return;
    }
    if accepted.host_id != host_id {
        update_warning.set(Some(UpdateDiagnostic {
            epoch,
            sticky: true,
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
        binding: plan.binding.clone(),
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
    pending: &'a mut Signal<Option<PendingPlan>>,
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
/// correlated success or removal clears what stands.
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
        pending,
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
        pending.set(None);
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
        text,
    }));
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
    /// Rows whose automatic disclosure currently holds them open. Published
    /// for update failures and unresolved diagnostics; the parent ORs this
    /// with the global checkbox per row.
    mut auto_details: Signal<HashSet<HostId>>,
    /// Compact row status published beside the automatic-disclosure set.
    mut row_update_progress: Signal<HashMap<HostId, HostUpdateProgress>>,
    /// Reveal global details when ADD preparation produces a confirmation or
    /// hidden diagnostic. Updates keep their running status inline and rely
    /// on automatic row disclosure only when a failure or uncertainty needs
    /// the full trace.
    on_reveal_details: EventHandler<()>,
    /// Paint-only busy bookkeeping for this row in the parent. It never
    /// excludes a run.
    on_running: EventHandler<bool>,
    /// Ask the authoritative registry to refresh after registration or an
    /// accepted run.
    on_changed: EventHandler<()>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let host_id = host.id;
    let binding = HostBinding::from(&host);
    let current_binding = use_memo(use_reactive((&binding,), |(binding,)| binding));
    let progress = use_signal(|| None::<ProvisioningView>);
    let progress_error = use_signal(|| None::<String>);
    let progress_surface = use_signal(SurfaceReader::default);
    let mut read_gate = use_signal(ReadGate::default);
    let mut pending = use_signal(|| None::<PendingPlan>);
    let mut planning = use_signal(|| false);
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
        let mut view_pending = pending;
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
                            pending: &mut view_pending,
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
    let begin_add_plan = move || -> bool {
        if *planning.peek() || ops.busy_now() {
            return false;
        }
        planning.set(true);
        pending.set(None);
        action_error.set(None);
        action_warning.set(None);
        let base = plan_base.clone();
        let host = plan_host.clone();
        let requested_binding = HostBinding::from(&host);
        let is_local_add = host.kind == HostKind::Local;
        let reread = plan_progress.clone();
        spawn(async move {
            let prepared = prepare(&base, &host, ProvisioningOperation::Add).await;
            if *current_binding.peek() != requested_binding {
                planning.set(false);
                return;
            }
            match prepared {
                Ok(Preparation::Plan(plan)) => {
                    if is_local_add {
                        local_auto_retry.set(true);
                    }
                    pending.set(Some(plan));
                    on_reveal_details.call(());
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
                    action_error.set(Some(reason));
                    on_reveal_details.call(());
                }
                Ok(Preparation::Unvalidated(problem)) => {
                    if is_local_add {
                        local_auto_retry.set(true);
                    }
                    action_error.set(Some(problem));
                    on_changed.call(());
                    on_reveal_details.call(());
                }
                Err(error) => {
                    if is_local_add {
                        local_auto_retry.set(true);
                    }
                    action_error.set(Some(error));
                    on_reveal_details.call(());
                }
            }
            planning.set(false);
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
        pending.set(None);
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
                            plan: Some(plan),
                        }));
                    } else {
                        // Defensive: only remote updates skip
                        // confirmation. The backend refuses local update
                        // plans, so this arm should not happen — but if
                        // it ever does, the plan confirms explicitly
                        // rather than submitting itself.
                        intent.set(None);
                        pending.set(Some(plan));
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
                    set_planning_diagnostic(&mut update_error, epoch_now, reason);
                }
                Ok(Preparation::Unvalidated(problem)) => {
                    intent.set(None);
                    set_planning_diagnostic(&mut update_error, epoch_now, problem);
                    on_changed.call(());
                }
                Err(error) => {
                    // Planning failure submits nothing; the diagnostic
                    // and the row's expansion stay behind it — unless a
                    // sticky uncertainty from an earlier submission
                    // outranks it (see `set_planning_diagnostic`).
                    intent.set(None);
                    set_planning_diagnostic(&mut update_error, epoch_now, error);
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
        let mut begin_add = begin_add_plan.clone();
        let mut begin_auto = begin_update;
        move |request: &ActionRequest| -> bool {
            // Snapshot the decision inputs as owned values first. A peek
            // guard borrowed into the match below would stay alive across
            // the planning sets and spawns in its arms, and that borrow
            // panics the wasm runtime (observed directly: two
            // `RuntimeError: unreachable` per click, zero requests sent).
            let current = current_binding.peek().clone();
            let owned = update_owned(intent.peek().as_ref(), tracked.peek().as_ref());
            let add_planning = *planning.peek();
            let busy = ops.busy_now();
            match decide_request(request, &current, owned, add_planning, busy) {
                RequestDecision::Refuse => false,
                RequestDecision::Stale | RequestDecision::Coalesce => true,
                RequestDecision::Plan => match request.operation {
                    ProvisioningOperation::Update => {
                        begin_auto();
                        true
                    }
                    ProvisioningOperation::Add => begin_add(),
                },
            }
        }
    };
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
        if live.binding != *current_binding.peek() || live.binding.kind != HostKind::Ssh {
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
    // target's history.
    let watcher_progress = request_progress.clone();
    use_effect(use_reactive((&binding,), move |(binding,)| {
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
        if target_changed {
            if let Some(followed) = followed {
                retired_runs.write().insert(followed.run_id.clone());
            }
            tracked.set(None);
            on_running.call(false);
            update_warning.set(Some(UpdateDiagnostic {
                epoch: *intent_epoch.peek(),
                sticky: true,
                text: "the host changed while its update was being followed, so that run's \
                     outcome is unknown; this row claims nothing from it"
                    .to_string(),
            }));
        }
    }));

    // Publish the row's disclosure and compact status together. Running work
    // remains folded unless the user opened global details; errors, warnings,
    // and retained failed runs keep the step list visible for diagnosis.
    use_effect(move || {
        let current_intent = intent();
        let current_tracked = tracked();
        let current_progress = progress();
        let error = update_error();
        let warning = update_warning();
        let read_error = progress_error();
        let disclosed = update_auto_disclosed(
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

        let row_progress = derive_row_update_progress(
            current_intent.as_ref(),
            current_tracked.as_ref(),
            current_progress.as_ref(),
        );
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
    let mut auto_plan = begin_add_plan.clone();
    use_effect(use_reactive(
        (&local_setup, &binding),
        move |(local_setup, binding)| {
            let authoritative = progress.read().clone();
            let page_busy = ops.busy();
            if pending
                .peek()
                .as_ref()
                .is_some_and(|plan| plan.binding != binding)
            {
                pending.set(None);
            }
            let running = authoritative.as_ref().is_some_and(|view| {
                view.run_id.is_some() && view.status == ProvisioningStatus::Running
            });
            if running {
                pending.set(None);
            }
            if local_setup
                && !page_busy
                && authoritative
                    .as_ref()
                    .is_some_and(|view| view.run_id.is_none())
                && !*local_probe_started.peek()
            {
                local_probe_started.set(true);
                auto_plan();
            }
            if !local_setup && *was_local_setup.peek() {
                local_probe_started.set(false);
                local_auto_retry.set(false);
                pending.set(None);
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
    use_effect(use_reactive(
        (&local_setup, &menu_host_kind),
        move |(local_setup, menu_host_kind)| {
            let snapshot = progress();
            let is_planning = planning();
            let has_pending_plan = pending().is_some();
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
            let update_allowed = menu_host_kind != HostKind::Unrecognized;
            let plan_in_flight = is_planning || has_pending_plan || owned;
            let next = ProvisioningMenuState {
                rerun: (update_allowed && !plan_in_flight)
                    .then_some(failed_operation)
                    .flatten(),
                automatic_setup: !run_active
                    && update_allowed
                    && local_setup
                    && !plan_in_flight
                    && failed_operation.is_none()
                    && can_retry_local_setup,
                update: !run_active && update_allowed && !local_setup && !plan_in_flight,
                planning: plan_in_flight,
            };
            if menu_states.peek().get(&host_id).copied() != Some(next) {
                menu_states.write().insert(host_id, next);
            }
        },
    ));

    // Explicit setup confirmation enters the same binding-checked,
    // single-consumption submission as automatic remote update. ADD
    // preparation may already have registered an answering supervisor, but
    // it never claims this token. The owned guard releases submission
    // exclusion even if this row disappears while the POST is pending.
    let submit_base = base.clone();
    let submit_progress = request_progress.clone();
    let confirm = move |_| {
        let Some(plan) = pending.peek().clone() else {
            return;
        };
        if plan.binding != *current_binding.peek() {
            pending.set(None);
            return;
        }
        let mut confirm_ops = ops;
        let Some(claim) = confirm_ops.claim_guard() else {
            return;
        };
        // Every attempt consumes what was displayed from the client's point
        // of view too. A refusal or transport ambiguity cannot prove the
        // helm left this one-use id unconsumed.
        pending.set(None);
        action_error.set(None);
        action_warning.set(None);
        if plan.binding.kind == HostKind::Local && plan.operation == ProvisioningOperation::Add {
            local_auto_retry.set(false);
        }
        submit_plan(
            Submission {
                base: submit_base.clone(),
                host_id,
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
                reread: submit_progress.clone(),
            },
            plan,
        );
    };

    let snapshot = progress.read().clone();
    let offer = pending.read().clone();
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
    let is_planning = *planning.read() || intent_phase == Some(IntentPhase::Planning);
    let is_waiting_claim = intent_phase == Some(IntentPhase::WaitingForClaim);
    let is_submitting = intent_phase == Some(IntentPhase::Submitting);
    let is_accepted_run_waiting = *accepted_run_waiting.read();
    let can_retry_local_setup = *local_auto_retry.read();
    let page_busy = ops.busy();
    let run_active = snapshot
        .as_ref()
        .is_some_and(|view| view.run_id.is_some() && view.status == ProvisioningStatus::Running);
    let failed_operation = snapshot.as_ref().and_then(|view| {
        (view.run_id.is_some() && view.status == ProvisioningStatus::Failed)
            .then_some(view.operation)
            .flatten()
    });
    // The folded row's one-line trace stands down while the host row shows
    // inline update status (see `derive_row_update_progress`); every other
    // running or failed retained run keeps it. Suppressing it here, where the trace
    // shape is also published, keeps fixed-surface geometry in step with
    // what is actually drawn.
    let inline_update_status = derive_row_update_progress(
        live_intent.as_ref(),
        tracked.read().as_ref(),
        snapshot.as_ref(),
    )
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
    let automatic_local_remedy = local_setup
        && (offer.is_some()
            || is_planning
            || run_active
            || is_accepted_run_waiting
            || failed_operation.is_some()
            || can_retry_local_setup);
    let has_details_content = snapshot.as_ref().is_some_and(|view| view.run_id.is_some())
        || offer.is_some()
        || is_planning
        || is_waiting_claim
        || is_submitting
        || current_error.is_some()
        || current_warning.is_some()
        || current_progress_error.is_some()
        || (local_setup && manual_remedy.is_some());

    rsx! {
        div {
            class: "provisioning-panel",
            "data-provisioning-host": "{host_id}",
            if details_open && has_details_content {
            div {
            if let Some(view) = snapshot && view.run_id.is_some() {
                div {
                        class: "provisioning-run",
                        "data-provisioning-status": "{status_label(view.status)}",
                        "data-provisioning-operation": "{operation_label(view.operation)}",
                        div { class: "provisioning-run-header",
                            span { class: "provisioning-title",
                                "{operation_label(view.operation)} provisioning"
                            }
                            span { class: "provisioning-status", "{status_label(view.status)}" }
                        }
                        ol { class: "provisioning-steps",
                            for step in view.steps {
                                li {
                                    class: "provisioning-step",
                                    "data-step": "{step.step}",
                                    "data-status": "{step.status}",
                                    span { class: "provisioning-step-name", "{step.step}" }
                                    span { class: "provisioning-step-status", "{step.status}" }
                                    if let Some(message) = step.message {
                                        PeerLine {
                                            class: "provisioning-step-message",
                                            parts: vec![DetailPart::Peer(message)],
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(message) = view.message {
                            PeerLine {
                                class: "provisioning-run-message",
                                parts: vec![DetailPart::Peer(message)],
                            }
                        }
                }
            }

            if let Some(plan) = offer {
                PlanConfirmation {
                    confirmation: plan.confirmation,
                    busy: page_busy,
                    confirm_label: match plan.operation {
                        ProvisioningOperation::Add => "confirm setup",
                        ProvisioningOperation::Update => "confirm update",
                    },
                    on_confirm: confirm,
                    on_cancel: move |_| {
                        if !ops.busy_now() {
                            pending.set(None);
                            action_error.set(None);
                            action_warning.set(None);
                            if local_setup {
                                local_auto_retry.set(true);
                            }
                        }
                    },
                }
            } else if is_planning {
                div {
                    class: "provisioning-planning",
                    if local_setup { "checking automatic setup…" } else { "planning…" }
                }
            } else if is_waiting_claim {
                // The minted plan is stored while the row stays folded;
                // submission follows as soon as the page token frees.
                div {
                    class: "provisioning-waiting",
                    "waiting to submit…"
                }
            } else if is_submitting {
                div {
                    class: "provisioning-submitting",
                    "submitting…"
                }
            }

            if let Some(error) = current_error {
                PeerLine {
                    class: "action-error provisioning-error",
                    parts: vec![DetailPart::Peer(error)],
                }
            }
            if let Some(warning) = current_warning {
                PeerLine {
                    class: "host-warning provisioning-warning",
                    parts: vec![DetailPart::Peer(warning)],
                }
            }
            if let Some(error) = current_progress_error {
                PeerLine {
                    class: "action-error provisioning-read-error",
                    parts: vec![DetailPart::Peer(error)],
                }
            }
            if local_setup && let Some(remedy) = manual_remedy {
                PeerLine {
                    class: if automatic_local_remedy {
                        "host-remedy provisioning-manual secondary"
                    } else {
                        "host-remedy provisioning-manual"
                    },
                    parts: remedy,
                }
            }
            }
            } else if let Some(shape) = visible_trace {
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
        None => "idle",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Only remote UPDATE skips confirmation. ADD keeps it in every shape,
    /// and so does anything that is not an ssh row — the menu's broader
    /// offer on local rows must never widen this gate.
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
    /// run continuity. A new id, kind, destination, or identity is a real
    /// target change, and old run evidence must not follow it.
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

    /// Inline progress counts terminal steps and names only a currently running step.
    ///
    /// The row cannot show the full trace while folded, so this summary must
    /// preserve executor order and reject stale snapshots that name another run.
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
                },
                crate::api::ProvisioningStep {
                    step: "upload-farhelm".to_string(),
                    status: "running".to_string(),
                    message: None,
                },
                crate::api::ProvisioningStep {
                    step: "install-supervisor".to_string(),
                    status: "pending".to_string(),
                    message: None,
                },
                crate::api::ProvisioningStep {
                    step: "optional-step".to_string(),
                    status: "skipped".to_string(),
                    message: None,
                },
                crate::api::ProvisioningStep {
                    step: "degraded-step".to_string(),
                    status: "degraded".to_string(),
                    message: None,
                },
                crate::api::ProvisioningStep {
                    step: "failed-step".to_string(),
                    status: "failed".to_string(),
                    message: None,
                },
                crate::api::ProvisioningStep {
                    step: "future-step".to_string(),
                    status: "new-status".to_string(),
                    message: None,
                },
            ],
            message: None,
        };
        let summary = update_progress_summary(&view, &tracked).expect("live run has progress");
        assert_eq!(summary.current_step.as_deref(), Some("upload-farhelm"));
        assert_eq!((summary.done, summary.total), (4, 7));
        assert_eq!(summary.started_at, started_at);

        let mut other_run = view.clone();
        other_run.run_id = Some("run-8".to_string());
        assert_eq!(update_progress_summary(&other_run, &tracked), None);
        let mut terminal = view;
        terminal.status = ProvisioningStatus::Failed;
        assert_eq!(update_progress_summary(&terminal, &tracked), None);
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
            derive_row_update_progress(Some(&intent(IntentPhase::Planning)), None, None),
            pending(UpdatePendingPhase::Planning)
        );
        assert_eq!(
            derive_row_update_progress(Some(&intent(IntentPhase::WaitingForClaim)), None, None),
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
            }],
            message: None,
        };
        // A live intent outranks whatever run and view are also present.
        assert_eq!(
            derive_row_update_progress(
                Some(&intent(IntentPhase::Submitting)),
                Some(&running_run),
                Some(&running_view)
            ),
            pending(UpdatePendingPhase::Submitting)
        );

        // Accepted but not yet read, or displaying some other run's view.
        let accepted = TrackedRun {
            last_status: None,
            ..running_run.clone()
        };
        assert_eq!(
            derive_row_update_progress(None, Some(&accepted), None),
            pending(UpdatePendingPhase::AwaitingProgress)
        );
        let mut other_view = running_view.clone();
        other_view.run_id = Some("run-8".to_string());
        assert_eq!(
            derive_row_update_progress(None, Some(&running_run), Some(&other_view)),
            pending(UpdatePendingPhase::AwaitingProgress)
        );

        let Some(HostUpdateProgress::Running(summary)) =
            derive_row_update_progress(None, Some(&running_run), Some(&running_view))
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
                derive_row_update_progress(None, Some(&terminal), None),
                None
            );
        }
        let add_run = TrackedRun {
            operation: Some(ProvisioningOperation::Add),
            ..running_run
        };
        assert_eq!(derive_row_update_progress(None, Some(&add_run), None), None);
        assert_eq!(
            derive_row_update_progress(None, None, Some(&running_view)),
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
            text: "an earlier explanation".to_string(),
        };
        assert_eq!(
            competing_run_warning(Some(&plain), 4, "another run started".to_string()),
            Some(UpdateDiagnostic {
                epoch: 4,
                sticky: false,
                text: "another run started".to_string(),
            }),
        );
        assert_eq!(
            competing_run_warning(None, 4, "another run started".to_string()),
            Some(UpdateDiagnostic {
                epoch: 4,
                sticky: false,
                text: "another run started".to_string(),
            }),
        );
    }
}

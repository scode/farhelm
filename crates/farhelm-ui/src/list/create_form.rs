//! The inline form for creating one session on a selected host.
//!
//! Intent binding and host reconciliation live beside the form because they
//! define when an idempotency key still describes the submitted create.

use dioxus::prelude::*;

use crate::api::{self, CreateAgent, ProfileCatalog, create_session, mint_intent_key};
use crate::feed::use_feed_reader;
use crate::ops::OpLock;
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::profiles::{
    AgentChoice, CatalogLookup, CatalogSurface, UNRESOLVED_VALUE, resolve_agent, seeded_choice,
    submitted_field,
};
use crate::reader::Trigger;
use crate::{
    ApiBase, HostId, LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection,
    ProfileExistence, Session,
};

use super::shared::{
    HostOption, OpenHost, effective_create_host, enrich_created_session, matching_host_option,
};

/// How many times one submit will mint a key before giving up.
///
/// The retry exists for a queued keystroke landing during the mint, which
/// resolves on the second attempt; anything beyond that is a form whose
/// values keep changing faster than a UUID can be generated, which is not a
/// create anybody is waiting on. Bounded rather than a bare loop because
/// spinning is a worse answer than saying so.
const MINT_ATTEMPTS: usize = 3;

/// Shared by visible invalidation and both submission checks so an explicit
/// destination correction can retire only this refusal, not an unrelated error.
const REMEMBERED_DESTINATION_CHANGED: &str = "the remembered folder belongs to a different installation; choose the host or folder again before launching";

/// The host installation a create intent is bound to.
///
/// Profile ids are helm-wide now, but the idempotency key and clone target are
/// still installation-specific: a registry row can be retargeted or adopted
/// while retaining its numeric id. This value keeps that safety boundary out
/// of the catalog model it no longer belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateTarget {
    pub(crate) host: HostId,
    fingerprint: String,
}

impl CreateTarget {
    /// Bind a registry row to the installation fingerprint the form observed.
    pub(crate) fn new(host: HostId, fingerprint: String) -> CreateTarget {
        CreateTarget { host, fingerprint }
    }
}

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
    /// A profile from the helm catalog, by id.
    Profile(String),
    /// Declarative structured intent compiled only by the helm.
    Structured(LaunchSelection),
}

/// The active creation surface. Legacy profiles/commands and structured
/// harnesses are separate modes because values from one cannot safely become
/// hidden inputs to the other's idempotency key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreationSurface {
    Legacy,
    Structured,
}

/// The stable HTML value for one effort choice.
///
/// The enum crosses HTTP as snake case, so the controls use the same literal
/// spelling instead of deriving a second display protocol from `Debug`.
fn effort_value(effort: LaunchEffort) -> &'static str {
    match effort {
        LaunchEffort::Low => "low",
        LaunchEffort::Medium => "medium",
        LaunchEffort::High => "high",
        LaunchEffort::Xhigh => "xhigh",
        LaunchEffort::Max => "max",
        LaunchEffort::Ultra => "ultra",
    }
}

/// Apply a clicked or keyboard-selected search result without launching.
///
/// Search is only a picker. Keeping its result application in one helper
/// makes Enter and pointer activation replace the same fields and clear the
/// same idempotency binding. A returned path means this was the explicit
/// browse action; the caller starts the shared guarded request after closing
/// the search surface.
#[expect(
    clippy::too_many_arguments,
    reason = "the signals are independent reactive ownership handles; bundling them would obscure which draft fields a search action may replace"
)]
fn apply_composer_search_result(
    result: crate::launch_composer::ComposerSearchResult,
    history_target: Option<CreateTarget>,
    live_destination: Signal<Option<CreateTarget>>,
    mut remembered_destination: Signal<Option<CreateTarget>>,
    history_activation_attempts: Signal<u64>,
    mut cwd: Signal<String>,
    mut cwd_raw_seed: Signal<Option<String>>,
    mut cwd_edited: Signal<bool>,
    mut structured_harness: Signal<Option<LaunchHarness>>,
    mut structured_model: Signal<Option<String>>,
    mut structured_model_raw_seed: Signal<Option<String>>,
    mut structured_model_edited: Signal<bool>,
    mut custom_model_harness: Signal<Option<LaunchHarness>>,
    mut structured_effort: Signal<Option<LaunchEffort>>,
    mut structured_permissions: Signal<Option<LaunchPermission>>,
    mut composer_reset_reason: Signal<Option<String>>,
    catalog: &[crate::api::LaunchCatalogModel],
    mut intent_key: Signal<Option<(String, IntentBinding)>>,
) -> Option<String> {
    use crate::launch_composer::ComposerSearchResult;
    if matches!(
        &result,
        ComposerSearchResult::Folder(_) | ComposerSearchResult::Recent(_)
    ) && !admit_history_destination(
        history_target,
        live_destination,
        remembered_destination,
        history_activation_attempts,
    ) {
        return None;
    }
    if matches!(
        &result,
        ComposerSearchResult::UsePath(_) | ComposerSearchResult::BrowsePath(_)
    ) {
        // An explicit path action replaces the remembered destination, even
        // when the person deliberately chooses the same spelling again.
        remembered_destination.set(None);
    }
    match result {
        crate::launch_composer::ComposerSearchResult::UsePath(folder)
        | crate::launch_composer::ComposerSearchResult::Folder(folder) => {
            reseed_cloned_field(&mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &folder);
        }
        crate::launch_composer::ComposerSearchResult::BrowsePath(folder) => {
            // Query-path actions are treated like other relayed path choices:
            // the escaped field remains reviewable, while the browse request
            // and an untouched later create retain the exact requested bytes.
            reseed_cloned_field(&mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &folder);
            intent_key.set(None);
            return Some(folder);
        }
        crate::launch_composer::ComposerSearchResult::Harness(harness) => {
            let before = LaunchSelection {
                harness: structured_harness().unwrap_or(harness),
                model: structured_model(),
                effort: structured_effort(),
                permissions: structured_permissions(),
            };
            let (selection, owner) = crate::launch_composer::reconcile_harness_selection(
                before.clone(),
                *custom_model_harness.peek(),
                harness,
                catalog,
            );
            composer_reset_reason.set(crate::launch_composer::reconciliation_reset_reason(
                &before, &selection,
            ));
            structured_harness.set(Some(selection.harness));
            structured_model_raw_seed.set(selection.model.clone());
            structured_model_edited.set(false);
            structured_model.set(selection.model);
            structured_effort.set(selection.effort);
            custom_model_harness.set(owner);
        }
        crate::launch_composer::ComposerSearchResult::Model { id, harness } => {
            let before = LaunchSelection {
                harness: structured_harness().unwrap_or(harness),
                model: structured_model(),
                effort: structured_effort(),
                permissions: structured_permissions(),
            };
            let selection = LaunchSelection {
                harness,
                model: Some(id),
                effort: structured_effort(),
                permissions: structured_permissions(),
            };
            let (selection, owner) = crate::launch_composer::reconcile_harness_selection(
                selection,
                Some(harness),
                harness,
                catalog,
            );
            composer_reset_reason.set(crate::launch_composer::reconciliation_reset_reason(
                &before, &selection,
            ));
            structured_harness.set(Some(selection.harness));
            structured_model_raw_seed.set(selection.model.clone());
            structured_model_edited.set(false);
            structured_model.set(selection.model);
            structured_effort.set(selection.effort);
            custom_model_harness.set(owner);
        }
        crate::launch_composer::ComposerSearchResult::Recent(entry) => {
            composer_reset_reason.set(None);
            let selection = crate::launch_composer::select_recent(&entry);
            let owner = selection.model.as_ref().and_then(|model| {
                (!catalog.iter().any(|candidate| candidate.id == *model))
                    .then_some(selection.harness)
            });
            structured_harness.set(Some(selection.harness));
            structured_model_raw_seed.set(selection.model.clone());
            structured_model_edited.set(false);
            structured_model.set(selection.model);
            structured_effort.set(selection.effort);
            structured_permissions.set(selection.permissions);
            custom_model_harness.set(owner);
            reseed_cloned_field(&mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &entry.cwd);
        }
    }
    intent_key.set(None);
    None
}

/// Refuse a draft transition once the shared operation token is held.
///
/// HTML's disabled state is rendered asynchronously, while a create claims
/// the token synchronously before its key-mint await. Every handler that can
/// alter the intent must consult this live predicate itself; otherwise an
/// already queued click could clear the key or change the selection the
/// accepted request is still resolving.
fn draft_transition_allowed(ops: OpLock) -> bool {
    !ops.busy_now()
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

/// Whether the create target still describes the selected row — the same
/// registry row and installation fingerprint.
///
/// Comparing the row id alone is not enough, and the gap is the documented
/// one-render lag: after a retarget or an adoption the `hosts` snapshot can
/// already describe the successor install while the derived target signal
/// still describes its predecessor. The full-target comparison keeps the
/// request's idempotency and connection claims bound to one installation.
///
/// Both sides absent is a match on purpose: the hostless refusal downstream
/// owns that case and says something more useful than "the target changed".
/// A selected row missing from the snapshot is a mismatch: no installation
/// target can describe a row that no longer exists.
fn target_matches_selection(
    selected: Option<HostId>,
    hosts: &[HostOption],
    target: Option<&CreateTarget>,
) -> bool {
    match (selected, target) {
        (None, None) => true,
        (Some(id), Some(target)) => hosts
            .iter()
            .find(|host| host.id == id)
            .is_some_and(|host| CreateTarget::new(host.id, host.incarnation.clone()) == *target),
        _ => false,
    }
}

/// The connection token a create names as its `expected_incarnation`, or
/// `None` when there is nothing to assert: the row is gone from the
/// snapshot, or it has never connected (`connection == 0`, the sentinel
/// `Host::incarnation` documents).
///
/// The sentinel must map to NO claim rather than a claim of zero. A host
/// making its FIRST connection between the form's snapshot and the helm's
/// routing has a real, nonzero token by then — a create carrying `0` would
/// be refused as stale even though this client never observed any
/// connection and had nothing to preserve.
fn connection_claim(hosts: &[HostOption], host: HostId) -> Option<u64> {
    hosts
        .iter()
        .find(|option| option.id == host)
        .and_then(|option| (option.connection != 0).then_some(option.connection))
}

/// Snapshot the installation a history reply is allowed to describe.
///
/// History is stored per installation, while a host selector keeps a stable
/// registry id across retargeting. Pairing the async response with this
/// target stops an old installation's suggestions from surviving into a
/// dialog that now names its successor.
fn history_target(hosts: &[HostOption], selected: Option<HostId>) -> Option<CreateTarget> {
    selected.and_then(|id| {
        hosts
            .iter()
            .find(|host| host.id == id)
            .map(|host| CreateTarget::new(host.id, host.incarnation.clone()))
    })
}

/// A remembered folder remains usable only on the installation it described.
///
/// Explicit host/text overrides carry no earlier remembered authority.
/// Reconnect tokens are deliberately absent from CreateTarget: reconnecting
/// to the same install must not revoke a path that still belongs to it.
fn remembered_destination_matches(
    remembered: Option<&CreateTarget>,
    current: Option<&CreateTarget>,
) -> bool {
    remembered.is_none() || remembered == current
}

/// Bind a historical selection before copying any of its fields into the draft.
///
/// A captured callback can outlive its offered DOM result. Check the current
/// rendered registry here, rather than letting withdrawal of suggestions stand
/// in for handler-time authority. The counter distinguishes refusal from a
/// callback that never ran in the mounted race regression.
fn admit_history_destination(
    expected: Option<CreateTarget>,
    live: Signal<Option<CreateTarget>>,
    mut remembered: Signal<Option<CreateTarget>>,
    mut attempts: Signal<u64>,
) -> bool {
    attempts.with_mut(|count| *count = count.wrapping_add(1));
    if expected.is_none() || expected.as_ref() != live.peek().as_ref() {
        return false;
    }
    remembered.set(expected);
    true
}

/// The full, live destination a directory reply is allowed to describe.
///
/// `CreateTarget` intentionally excludes a reconnect token because create
/// idempotency is bound to an installation, not one supervisor connection.
/// Browsing is different: a directory listing is a live observation, so its
/// authority also expires when that connection is replaced under the same
/// registry row.
#[derive(Debug, Clone, PartialEq, Eq)]
struct BrowseAuthority {
    target: CreateTarget,
    connection: Option<u64>,
    cwd: String,
    generation: u64,
}

/// Clear every trace of a directory observation before changing destination.
///
/// Every host or folder transition takes this route. Incrementing generation
/// as well as dropping the request makes an old A listing in an A→B→A trip
/// ineligible even if a queued handler still holds its rendered result.
fn invalidate_directory_browse(
    mut browse_generation: Signal<u64>,
    mut browse_request: Signal<Option<BrowseAuthority>>,
    mut browse_result: Signal<Option<(BrowseAuthority, api::DirectoryBrowse)>>,
    mut browse_error: Signal<Option<String>>,
) {
    browse_generation.with_mut(|generation| *generation = generation.wrapping_add(1));
    browse_request.set(None);
    browse_result.set(None);
    browse_error.set(None);
}

/// Make the latest fetched suggestions visible after an intentional choice.
///
/// A picker action is the contract boundary where refreshed ranking may
/// appear. The target check keeps a late predecessor history response from
/// becoming the first set of suggestions for a newly selected host.
fn promote_history_snapshot(
    mut offered: Signal<Option<(CreateTarget, api::LaunchHistory)>>,
    current_target: Option<CreateTarget>,
    fetched: Option<(CreateTarget, api::LaunchHistory)>,
) {
    if let Some((target, history)) = fetched
        && Some(&target) == current_target.as_ref()
    {
        offered.set(Some((target, history)));
    }
}

/// Promote the most recently fetched history only when a person chooses.
///
/// The fetched signal is updated by the resource path, but never rendered
/// directly. Keeping that handoff separate makes a feed update available to
/// every deliberate ordinary choice without turning the update itself into a
/// surprise reorder of an open composer.
fn promote_fetched_history_snapshot(
    offered: Signal<Option<(CreateTarget, api::LaunchHistory)>>,
    current_target: Signal<Option<CreateTarget>>,
    fetched: Signal<Option<(CreateTarget, api::LaunchHistory)>>,
) {
    promote_history_snapshot(offered, current_target(), fetched());
}

/// Decide whether an asynchronous directory reply still belongs to this form.
///
/// The request must still be the current generation, and the live form must
/// still name the same installation, connection, and raw path. Callers use
/// this exact predicate at completion, rendering, and activation so a queued
/// click cannot apply a result that disappeared between those phases.
fn browse_reply_is_current(
    current: &Option<BrowseAuthority>,
    candidate: &BrowseAuthority,
    live_target: Option<CreateTarget>,
    live_connection: Option<u64>,
    live_cwd: &str,
) -> bool {
    current.as_ref() == Some(candidate)
        && live_target.as_ref() == Some(&candidate.target)
        && live_connection == candidate.connection
        && live_cwd == candidate.cwd
}

/// Recheck browse authority from an event handler, after its node rendered.
fn browse_activation_is_current(
    request: Signal<Option<BrowseAuthority>>,
    candidate: &BrowseAuthority,
    target: Signal<Option<CreateTarget>>,
    connection: Signal<Option<u64>>,
    cwd: Signal<String>,
    raw_seed: Signal<Option<String>>,
    edited: Signal<bool>,
) -> bool {
    browse_reply_is_current(
        &request(),
        candidate,
        target(),
        connection(),
        &submitted_field(&cwd(), edited(), raw_seed.peek().as_deref()),
    )
}

/// Start one directory listing bound to the current create destination.
///
/// Both the ordinary Browse button and search's Browse-this-path action use
/// this entry point so neither can accidentally weaken the target,
/// connection, path, or generation guard. The request is explicit: callers
/// choose when to invoke it; merely updating the search query never reaches
/// the remote filesystem.
#[expect(
    clippy::too_many_arguments,
    reason = "the browse authority intentionally receives separate live signals so its completion guard cannot retain a stale aggregate draft"
)]
fn request_directory_browse(
    base: String,
    selected: Option<HostId>,
    hosts: &[HostOption],
    browse_target: Signal<Option<CreateTarget>>,
    requested_cwd: String,
    mut browse_generation: Signal<u64>,
    mut browse_request: Signal<Option<BrowseAuthority>>,
    mut browse_result: Signal<Option<(BrowseAuthority, api::DirectoryBrowse)>>,
    mut browse_error: Signal<Option<String>>,
    mut reply_completions: Signal<u64>,
    live_connection: Signal<Option<u64>>,
    live_cwd: Signal<String>,
    live_cwd_raw_seed: Signal<Option<String>>,
    live_cwd_edited: Signal<bool>,
) {
    let Some(host) = selected else { return };
    let Some(target) = history_target(hosts, Some(host)) else {
        return;
    };
    let generation = browse_generation().wrapping_add(1);
    browse_generation.set(generation);
    let authority = BrowseAuthority {
        target,
        connection: connection_claim(hosts, host),
        cwd: requested_cwd.clone(),
        generation,
    };
    browse_request.set(Some(authority.clone()));
    browse_result.set(None);
    browse_error.set(None);
    spawn(async move {
        let result = api::browse_directory(&base, host, &requested_cwd, authority.connection).await;
        reply_completions.with_mut(|completions| *completions = completions.wrapping_add(1));
        let current = browse_request.peek().clone();
        if !browse_reply_is_current(
            &current,
            &authority,
            browse_target(),
            live_connection(),
            &submitted_field(
                &live_cwd(),
                live_cwd_edited(),
                live_cwd_raw_seed.peek().as_deref(),
            ),
        ) {
            return;
        }
        match result {
            Ok(result) => browse_result.set(Some((authority, result))),
            Err(reason) => browse_error.set(Some(reason)),
        }
    });
}

/// Install the dialog-local Tab loop after the browser owns the mounted form.
///
/// The composer is rendered inside the sidebar rather than in a portal, so
/// `aria-modal` alone cannot stop native Tab navigation from reaching the
/// still-mounted page behind it. Keeping the listener on this particular DOM
/// node makes its lifetime exactly the dialog's lifetime; no global handler
/// can survive a close and interfere with the next open.
fn install_composer_focus_trap() {
    document::eval(
        r#"(() => {
            const dialog = document.querySelector('.create-session-form[role="dialog"]');
            if (!dialog || dialog.__farhelmComposerFocusTrap) return;
            dialog.__farhelmComposerFocusTrap = true;
            const focusable = () => [...dialog.querySelectorAll(
                'button:not([disabled]), input:not([disabled]), select:not([disabled]), summary, [tabindex]:not([tabindex="-1"])',
            )].filter((node) => !node.hidden && node.getClientRects().length);
            dialog.addEventListener('keydown', (event) => {
                if (event.key !== 'Tab') return;
                const nodes = focusable();
                if (!nodes.length) return;
                const first = nodes[0];
                const last = nodes[nodes.length - 1];
                if (event.shiftKey ? document.activeElement === first : document.activeElement === last) {
                    event.preventDefault();
                    const target = event.shiftKey ? last : first;
                    // A trapped dialog is also its own scroll viewport. A
                    // boundary wrap that leaves the new focus above or below
                    // that viewport is technically contained but unusable:
                    // the focus ring and the control it names have vanished.
                    // Native focus scrolling is intentionally retained here.
                    target.focus();
                }
            });
        })();"#,
    );
}

/// Give each composer surface an explicit first focus target.
///
/// A structured dialog starts at its searchable chooser. Legacy clones do
/// not render that control, so they start at the agent selector instead of
/// leaving focus on the page beneath the modal. This runs on a deliberate
/// surface transition too, which keeps the same contract when Other and Back
/// swap the controls after the dialog has mounted.
fn focus_composer_surface() {
    document::eval(
        r#"(() => {
            const dialog = document.querySelector('.create-session-form[role="dialog"]');
            if (!dialog) return;
            const target = dialog.querySelector(
                '.launch-composer-search input:not([disabled]), .create-session-agent:not([disabled])'
            );
            target?.focus({ preventScroll: true });
        })()"#,
    );
}

/// Keep keyboard selection visible while a long combobox result list scrolls.
///
/// The input retains browser focus for combobox semantics, so this explicitly
/// scrolls the active option rather than moving focus into a transient button.
fn scroll_composer_search_result(index: usize) {
    document::eval(&format!(
        r#"document.getElementById('launch-composer-search-option-{index}')?.scrollIntoView({{ block: 'nearest' }});"#,
    ));
}

/// Which of the two creation modes a "clone" click's snapshot TRUSTS —
/// deliberately not carrying its own payload; see [`CreatePrefill::invocation`]
/// for where that lives and why.
///
/// The choice between the two variants is [`prefill_from`]'s to make: a
/// clone trusts the row's profile id ONLY when its own `source_profile` says
/// `Present` — the catalog, as of the row's own snapshot, still holds that
/// id under the SAME name. Every other answer — no profile at all, a name
/// the catalog has since changed, an id it no longer holds at all, or an
/// existence word this build does not recognize — falls back to the raw
/// invocation instead.
///
/// This is DELIBERATELY STRICTER than `profiles::resolve_agent`'s own rule
/// for an ordinary create, not a restatement of it: `resolve_agent` accepts
/// a previously chosen profile id whenever the catalog still holds it AT
/// ALL, name changes included, because that choice was made by a human
/// looking at today's picker a moment ago. A clone's row can be arbitrarily
/// old, so "the id still exists" is not enough evidence that cloning it
/// again is what a look at today's catalog would still choose — a rename is
/// exactly the kind of change SPEC.md's own snapshot rule says a session
/// must not be silently re-bound across, and a clone re-selecting the same
/// id under new user-visible clothing would be doing precisely that.
///
/// Trusting the id at all, even under `Present`, is still a SNAPSHOT
/// decision, not a live one: once a profile-backed clone is applied and the
/// user submits, the request names the id and nothing else, and the
/// helm resolves it against whatever definition the catalog holds AT SUBMIT
/// TIME — an edit landing between the clone click and the submit
/// changes what the cloned session launches, exactly as it would for any
/// other profile-backed create, and a deletion in that window is refused by
/// either the refreshed picker or the helm rather than falling back.
///
/// Not [`LaunchIntent`] reused, even though the two-mode split is
/// identical: `LaunchIntent::Command(String)` pairs its variant with the
/// launch string, but a clone's raw invocation has to reach the form's
/// command field in EITHER mode (see `CreatePrefill::invocation`) — parking
/// it inside a `Command` payload here would duplicate that string when the
/// mode already is command, and leave a `Profile`-backed clone with nowhere
/// on `LaunchIntent` to carry it at all. A bare marker beside one shared
/// field says the same thing without either problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PrefillAgent {
    /// Launch the row's own profile again, by id, on the row's own host.
    Profile { id: String },
    /// No profile this clone may trust — launch the raw invocation
    /// (`CreatePrefill::invocation`) verbatim instead.
    Command,
}

/// What one "clone" click seeds a fresh create form with: everything about
/// the clicked row that a NEW session can reuse.
///
/// Built once, by [`prefill_from`], from the row's `Session` at the moment
/// of the click — a snapshot, not a live binding, which is what SPEC.md's
/// profile-snapshot rule already requires of `Session::source_profile`
/// itself: a profile edited or deleted after the click must not reach back
/// into an open, already-prefilled form.
///
/// `title` and `cwd` travel verbatim, duplicate title included. SPEC.md's
/// creation rule has nothing to say about uniqueness, an identical title is
/// one rename away from being fixed, and inventing a "(copy)" suffix would
/// be this UI's own opinion about a field the user never asked it to guess
/// at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CreatePrefill {
    /// Bumped by every clone click, including a second clone of the SAME
    /// row. `CreateSessionForm`'s reseed effect compares this against
    /// `prefill_applied` rather than mere presence, which is what makes
    /// cloning one row twice in a row reseed the form the second time too,
    /// instead of that click being a silent no-op because a prefill was
    /// already on screen.
    pub(super) generation: u64,
    /// The row's host, so the form's selector follows the CLONED session
    /// rather than whatever the dialog last had chosen. `None` only for a
    /// row from a helm old enough to omit `Session::host` entirely, in
    /// which case the form leaves the selector alone and falls back to its
    /// ordinary default precedence (`effective_create_host`) — the same
    /// degradation every other host-carrying field on `Session` already
    /// accepts from such a peer.
    pub(super) host: Option<HostId>,
    /// The install identity the row reported alongside `host`, straight off
    /// `Session::host_identity` — see that field's own doc for the double-
    /// `Option` contract. This is what lets the reseed effect tell a row
    /// whose host still fronts the SAME install apart from one that has
    /// since been retargeted or adopted onto a successor: accepting the
    /// latter's host id at face value would send this clone's launch to a
    /// machine the row no longer actually names — exactly the
    /// #156-style residual `shared::matching_host_option` already closes
    /// for the ordinary create default, reused here for the same question.
    pub(super) host_identity: Option<Option<String>>,
    pub(super) cwd: String,
    pub(super) title: String,
    /// The row's raw launch command, ALWAYS carried regardless of which
    /// mode [`agent`](Self::agent) trusts.
    ///
    /// This is the seed for custom-command mode. While a profile is selected,
    /// the disabled field displays that profile's current invocation instead;
    /// retaining the raw value here keeps switching to custom mode faithful to
    /// the cloned row without making the profile-mode display look executable.
    pub(super) invocation: String,
    /// Declarative provenance for a structured source session.
    ///
    /// This remains absent for legacy rows. Clone must preserve that absence
    /// rather than reverse-engineering a harness from an arbitrary command.
    pub(super) launch: Option<LaunchSelection>,
    pub(super) agent: PrefillAgent,
}

/// Build the prefill a clone click seeds the create form with, from the
/// clicked row's own `Session`.
///
/// Kept as a pure mapping — apart from `CreateSessionForm` and from whatever
/// mints `generation` (`list::view::ListView`, once per clone click) — so
/// the Present/Renamed/Deleted/Unrecognized/None decision (see
/// [`PrefillAgent`]) is checkable without mounting a component.
pub(super) fn prefill_from(session: &Session, generation: u64) -> CreatePrefill {
    let agent = match &session.source_profile {
        Some(source) if source.existence == ProfileExistence::Present => PrefillAgent::Profile {
            id: source.id.clone(),
        },
        _ => PrefillAgent::Command,
    };
    CreatePrefill {
        generation,
        host: session.host,
        host_identity: session.host_identity.clone(),
        cwd: session.cwd.clone(),
        title: session.title.clone(),
        invocation: session.invocation.clone(),
        launch: session.launch.clone(),
        agent,
    }
}

// ---------------------------------------------------------------------
// A clone's host and agent choices, reconciled on independent lifecycles
// ---------------------------------------------------------------------

/// What has become of the current clone generation's host binding.
///
/// Host identity remains installation-specific even though the agent catalog
/// is helm-wide. Keeping this state about the host alone prevents a delayed
/// registry answer or later retarget from owning an agent choice that remains
/// valid on every host.
///
/// Four states rather than a bool, because three different questions later
/// code needs answered would otherwise collapse into one flag that cannot
/// distinguish them: "is there still something to try automatically",
/// "does `chosen_host` currently hold THIS generation's own pick, subject
/// to being withdrawn", and "has this decision already been taken away
/// from automatic handling, whether because it failed or because a human
/// took over".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloneHostState {
    /// Still trying: either the registry has not answered at all yet
    /// (item2-review2.md F1 — a clone opened before the first `hosts` read
    /// lands must not give up permanently, since `prefill_applied` is
    /// already latched by the time the registry answers and will never
    /// send this generation through the ordinary reseed branch again), or
    /// this pass is the first chance to check a registry that already had.
    Waiting,
    /// `chosen_host` currently holds this generation's own pick.
    /// Re-checked every pass: the moment `matching_host_option` stops
    /// confirming the source installation — a retarget or an adopt lands
    /// while the form stays open — the binding is withdrawn back to
    /// `Unconfirmable` (item2-review2.md F3), rather than silently
    /// continuing to name a machine the clone was never actually taken
    /// from.
    Bound,
    /// Nothing left for the automatic resolver to try: a hostless
    /// (legacy) row, whose install can never be confirmed at all
    /// (item2-review2.md F4); a hostful row whose identity check failed
    /// once the registry had a chance to answer; or a `Bound` binding that
    /// was just withdrawn. `chosen_host` is left to its ordinary, non-clone
    /// rules from here — for the REST of this
    /// generation's lifetime, since only a fresh clone (a new generation)
    /// re-seeds `Waiting`.
    Unconfirmable,
    /// An explicit host interaction (the selector's own `onchange`) has
    /// taken this generation's host decision away from automatic handling
    /// entirely (item2-review2.md F6's spirit): the user picked a
    /// host with their own hand, so a later retarget of the CLONE's row
    /// must not silently pull the rug out from under a choice the clone
    /// had nothing to do with anymore. Ordinary (non-clone) retarget
    /// handling — clearing the agent, rotating the intent key — still
    /// applies; only this file's STRONGER clone-specific withdrawal does
    /// not.
    UserTookOver,
}

/// What the resolver has decided to DO this pass, given a [`CloneHostState`]
/// and the registry as it currently stands.
///
/// Kept apart from `CloneHostState` itself (rather than folding the action
/// into the `Bound` variant, say) because the STATE is what persists across
/// passes and the ACTION is a one-shot instruction for THIS pass only —
/// conflating them would leave it ambiguous whether a stored action should
/// be replayed on the next unrelated render.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CloneHostAction {
    /// Nothing to do this pass — the caller's signals are untouched.
    Hold,
    /// Apply the clone's confirmed host installation.
    Bind(CreateTarget),
    /// Undo a binding this generation had previously made.
    Withdraw,
}

/// Resolve one pass of a clone's host binding as a pure decision.
///
/// This keeps item2-review2.md's F1 (retry once the registry loads), F3
/// (withdraw the instant a bound installation stops matching), and F4 (a
/// hostless clone never invents a host) deterministic and checkable without
/// mounting a component or an effect. Agent seeding is deliberately absent;
/// [`resolve_clone_agent`] owns its independent helm-catalog lifecycle.
///
/// `chosen_host_is_bound_host` is only consulted in the `Bound` state: it
/// is how the caller reports that `chosen_host` has moved away from this
/// generation's own pick since the last pass (an explicit re-selection is
/// what usually causes that, and the host `<select>`'s own `onchange`
/// already transitions to `UserTookOver` directly for that ordinary case —
/// this is the belt to that handler's braces, covering any other path that
/// might move `chosen_host` without going through it).
fn resolve_clone_host(
    state: CloneHostState,
    prefill_host: Option<HostId>,
    prefill_identity: &Option<Option<String>>,
    hosts_loaded: bool,
    hosts: &[HostOption],
    chosen_host_is_bound_host: bool,
) -> (CloneHostState, CloneHostAction) {
    match state {
        CloneHostState::Waiting => {
            let Some(host) = prefill_host else {
                // A hostless clone starts life straight in `Unconfirmable`
                // (see the caller), so reaching `Waiting` with no host here
                // is unreachable in practice — handled rather than
                // `unreachable!()`, since this function's whole point is to
                // be checkable in isolation from that caller invariant.
                return (CloneHostState::Unconfirmable, CloneHostAction::Hold);
            };
            if !hosts_loaded {
                // F1: keep waiting rather than giving up — the caller's
                // `prefill_applied` latch means this is the ONLY chance
                // left to apply this generation's host once the registry
                // does answer.
                return (CloneHostState::Waiting, CloneHostAction::Hold);
            }
            let open = OpenHost {
                id: host,
                identity: prefill_identity.clone(),
            };
            match matching_host_option(&open, hosts) {
                Some(option) => {
                    let target = CreateTarget::new(option.id, option.incarnation.clone());
                    (CloneHostState::Bound, CloneHostAction::Bind(target))
                }
                // The registry has answered and this row's install cannot
                // be confirmed — permanently, for the rest of this
                // generation's lifetime; only a fresh clone tries again.
                None => (CloneHostState::Unconfirmable, CloneHostAction::Hold),
            }
        }
        CloneHostState::Bound => {
            if !chosen_host_is_bound_host {
                // The selector has moved on without going through
                // `Withdraw` — an explicit re-pick that reached
                // `chosen_host` some other way than the handler that
                // already transitions this directly. Nothing further to
                // automatically manage either way.
                return (CloneHostState::UserTookOver, CloneHostAction::Hold);
            }
            let Some(host) = prefill_host else {
                // Structurally unreachable: `Bound` is only ever entered
                // from a hostful `Waiting` resolution above.
                return (CloneHostState::Unconfirmable, CloneHostAction::Withdraw);
            };
            let open = OpenHost {
                id: host,
                identity: prefill_identity.clone(),
            };
            if matching_host_option(&open, hosts).is_some() {
                (CloneHostState::Bound, CloneHostAction::Hold)
            } else {
                // F3: the row's install no longer matches what it was
                // cloned from — withdraw rather than let the selector keep
                // silently naming a machine the clone was never actually
                // taken from.
                (CloneHostState::Unconfirmable, CloneHostAction::Withdraw)
            }
        }
        CloneHostState::Unconfirmable | CloneHostState::UserTookOver => {
            (state, CloneHostAction::Hold)
        }
    }
}

/// Whether a clone may still seed its agent choice automatically.
///
/// Unlike host reconciliation, this is one-shot. The helm-wide catalog can
/// confirm a profile without knowing anything about the source host, and no
/// later host event may withdraw or replace the result. An explicit picker or
/// command interaction permanently takes authority for this clone generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloneAgentState {
    /// The source choice has not yet been applied, normally because its
    /// profile still needs confirmation from the first catalog answer.
    Waiting,
    /// The source choice was applied once and must not be replayed.
    Seeded,
    /// A person chose the agent and automatic seeding must stand down.
    UserTookOver,
}

/// Resolve one pass of clone-agent seeding against the helm catalog.
///
/// Raw commands need no catalog evidence and seed immediately. A profile id
/// waits for a catalog, uses the profile only while that catalog still holds
/// it, and otherwise falls back to the source session's raw command. Terminal
/// states return no action so unrelated renders cannot overwrite the choice.
fn resolve_clone_agent(
    state: CloneAgentState,
    prefill: &PrefillAgent,
    catalog: Option<&ProfileCatalog>,
) -> (CloneAgentState, Option<AgentChoice>) {
    if state != CloneAgentState::Waiting {
        return (state, None);
    }
    match prefill {
        PrefillAgent::Command => (CloneAgentState::Seeded, Some(AgentChoice::Command)),
        PrefillAgent::Profile { id } => {
            let Some(catalog) = catalog else {
                return (CloneAgentState::Waiting, None);
            };
            let choice = if catalog.profiles.iter().any(|profile| profile.id == *id) {
                AgentChoice::Profile(id.clone())
            } else {
                AgentChoice::Command
            };
            (CloneAgentState::Seeded, Some(choice))
        }
    }
}

/// Seed one of the create form's peer-relayed text fields (working
/// directory, invocation, title) from a clone's raw value: shown escaped
/// (`peer::display_peer`), with the exact raw bytes and a cleared edited
/// flag recorded alongside so an untouched submit reads back the ORIGINAL
/// bytes rather than the escaped spelling (item2-review2.md F5;
/// `profiles::submitted_field` is the read-back half, and
/// `profiles::ProfileDraft::of` is this exact model's other caller).
fn reseed_cloned_field(
    display: &mut Signal<String>,
    raw_seed: &mut Signal<Option<String>>,
    edited: &mut Signal<bool>,
    raw: &str,
) {
    display.set(display_peer(raw));
    raw_seed.set(Some(raw.to_string()));
    edited.set(false);
}

/// The session-launch dialog, including the structured composer and the
/// explicit legacy fallback.
///
/// A new dialog opens on the structured surface with no harness selected.
/// Profiles and arbitrary commands remain available through the legacy
/// surface, but cannot contribute hidden values to a structured request.
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
/// The dialog offers the helm's whole catalog on every host and defaults to
/// the helm-wide last-used profile, asking rather than guessing when that
/// profile is gone. `profiles::resolve_agent` owns both halves of that rule.
/// Changing the host leaves an explicit choice intact. The command field is
/// disabled while a profile is selected, because the
///   two creation modes are mutually exclusive on the wire and a body naming
///   both is refused. Disabling it is also what keeps the intent binding
///   honest: a field the user cannot reach cannot change what the key stands
///   for.
///
/// The raw command path stays on the dialog rather than being replaced. It is
/// what runs anything no profile describes, and the e2e harness's own creates
/// go through it — a dialog that only offered profiles would make an ad-hoc
/// command a trip to `curl`.
///
/// ## Clone prefill
///
/// A "clone" click (`ListView`'s `on_clone`) opens this same form with
/// `prefill` set instead of building a second, immediate-create path — see
/// `CreatePrefill`'s own doc for what it carries and why. The one wrinkle it
/// adds to the agent picker: a prefilled profile choice must WIN over the
/// helm's remembered default on the render right after the clone,
/// which is the opposite of the ordinary "nothing chosen yet, seed from the
/// remembered default" rule two paragraphs up. The reseed effect (below)
/// gets this for free rather than as a special case, because it sets
/// `chosen_profile` before the remembered-default seeding runs, and that
/// seeding already backs off the moment a choice already exists. Profile
/// confirmation is independent of host reconciliation: the one helm catalog
/// applies everywhere, so a delayed or unconfirmable host cannot suppress a
/// valid source profile, and a later host answer cannot overwrite a person's
/// explicit agent choice.
///
/// The host is not accepted at face value. A `HostId` is a registry row
/// that survives a retarget or an adopt while the machine behind it
/// changes, so the reseed effect runs the cloned row's `host` and
/// `host_identity` through `shared::matching_host_option` — the exact
/// install comparison `default_create_host` already applies to SPEC.md's
/// ordinary creation default — before selecting it (`resolve_clone_host`, and
/// `CloneHostState`'s own doc for the states that comparison moves between).
/// A row whose install
/// cannot be confirmed — hostless entirely, or mismatched once the
/// registry has had a chance to answer — is left unselected: the selector
/// falls through to its ordinary default, while the agent is still seeded
/// from the helm catalog; `clone_host_note` (below) tells the user why the
/// host changed, reusing
/// the same host-note slot `choice_vanished` already renders through.
/// Unconfirmed is not the same as unchecked, though: a clone opened before
/// the FIRST `hosts` read lands is retried once the registry answers
/// rather than given up on (item2-review2.md F1) — necessary because the
/// text-field reseed above is a one-shot latch and will not give the host
/// a second chance on its own — and a clone whose binding DID succeed is
/// re-checked on every later pass too, so a retarget or an adopt landing
/// while the form stays open withdraws it back to unselected rather than
/// silently continuing to name a machine the clone was never actually
/// taken from (F3). An explicit host pick takes the decision away from
/// this reconciliation entirely, permanently, for the rest of the
/// generation (`CloneHostState::UserTookOver`). The derived target may catch
/// up one render after `chosen_host`, but it gates host identity and
/// idempotency only; it does not scope the catalog or clear the agent choice.
#[component]
pub(super) fn CreateSessionForm(
    hosts: Vec<HostOption>,
    /// The selected session's host and reported installation identity,
    /// carried by `ListView`'s `open_destination` snapshot. This supplies
    /// SPEC.md's first create-default clause independently of sidebar filtering.
    open_host: Option<OpenHost>,
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
    /// `ListView`'s signal rather than this form's so the same effective-host
    /// derivation also binds the idempotency key and connection claim.
    mut chosen_host: Signal<Option<HostId>>,
    /// The installation currently selected for the create. This is separate
    /// from the helm-wide catalog and exists only for host safety checks.
    create_target: Signal<Option<CreateTarget>>,
    /// The helm-wide profile catalog shared with the profiles popup.
    catalog: CatalogSurface,
    /// The page's live-operation token. Claimed at submit, released when the
    /// request completes — the exclusion against every host mutation, and
    /// against a second submit of this form (see `ops`).
    mut ops: OpLock,
    /// Directory copied from the currently selected session for an ordinary
    /// New action. `None` preserves the portable target-home default.
    initial_cwd: Option<String>,
    /// A "clone" click's seed, or `None` for the ordinary blank-form open.
    /// `ListView` owns the signal this reads and bumps `generation` on
    /// every clone (see `CreatePrefill`); this component's own reseed
    /// effect (below `chosen_profile`'s declaration) is what turns a new
    /// generation into field values.
    prefill: Option<CreatePrefill>,
    /// Discard this draft without creating a session.
    on_cancel: EventHandler<()>,
    on_created: EventHandler<Session>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let launch_catalog_base = base.clone();
    let launch_catalog = use_resource(move || {
        let base = launch_catalog_base.clone();
        async move { api::fetch_launch_catalog(&base).await }
    });
    // Prefilled rather than empty-with-a-placeholder, deliberately: what
    // gets sent is always exactly what the field shows, and the common
    // "just give me a session in my home directory" create needs no typing
    // at all. `~` resolves against the TARGET host's home — the supervisor
    // expands it at create time (SPEC.md's working-directory rule), which
    // is what makes a host-independent default possible here at all: this
    // form cannot know a remote host's home path.
    let cwd_initial_seed = initial_cwd.clone();
    let mut cwd = use_signal(move || {
        cwd_initial_seed
            .as_deref()
            .map(display_peer)
            .unwrap_or_else(|| "~".to_string())
    });
    let mut invocation = use_signal(String::new);
    let mut title = use_signal(String::new);
    let mut creation_surface = use_signal(|| CreationSurface::Structured);
    let mut structured_harness = use_signal(|| None::<LaunchHarness>);
    let mut structured_model = use_signal(|| None::<String>);
    // A restored custom id is peer text even though it is stored in a launch
    // selection. Keep its raw bytes for submission while showing an escaped
    // spelling until the person deliberately edits the field.
    let mut structured_model_raw_seed = use_signal(|| None::<String>);
    let mut structured_model_edited = use_signal(|| false);
    let mut custom_model_harness = use_signal(|| None::<LaunchHarness>);
    let mut structured_effort = use_signal(|| None::<LaunchEffort>);
    let mut structured_permissions = use_signal(|| None::<LaunchPermission>);
    // Compatibility clears are intentional, but defaults must never make an
    // earlier explicit choice vanish without telling the person what changed.
    let mut composer_reset_reason = use_signal(|| None::<String>);
    let mut composer_search = use_signal(String::new);
    let mut composer_search_open = use_signal(|| false);
    let mut composer_search_index = use_signal(|| 0_usize);
    // What each of the three text fields above was SEEDED from, raw, and
    // whether the user has typed in it since — `profiles::ProfileDraft`'s
    // escaped-display / raw-seed / edited-flag model, reused rather than
    // re-invented (item2-review2.md F5): a clone's directory, invocation and
    // title are peer-relayed text going into an editable control for
    // exactly the reason a profile's name and invocation are, and an
    // untouched field must submit the ORIGINAL bytes rather than the
    // escaped spelling `cwd`/`invocation`/`title` display while the clone is
    // on screen (see the reseed effect below for where the escaping is
    // applied, and `profiles::submitted_field` for the read-back half these
    // are fed into at submit time). `None` seeds mean "never clone-seeded",
    // which is the ordinary blank-create case: there the field's own text
    // already IS the value to submit, since nothing relayed it from a peer.
    let cwd_initial_raw_seed = initial_cwd.clone();
    let mut cwd_raw_seed = use_signal(move || cwd_initial_raw_seed);
    let mut cwd_edited = use_signal(|| false);
    let mut invocation_raw_seed = use_signal(|| None::<String>);
    let mut invocation_edited = use_signal(|| false);
    let mut title_raw_seed = use_signal(|| None::<String>);
    let mut title_edited = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    // The agent this dialog is going to use, once anything has decided it —
    // the user picking, or the helm's remembered default being CONSUMED (see
    // the effect below). `None` means nobody has decided yet.
    let mut chosen_profile = use_signal(|| None::<AgentChoice>);
    // Whether an explicit choice has been overtaken by reality. Derived per
    // render rather than written back into `chosen_host`, so it cannot
    // outlive the condition that produced it — and so a host that comes back
    // (a re-added destination) silently reinstates the user's choice.
    let choice_vanished =
        chosen_host().is_some_and(|chosen| !hosts.iter().any(|host| host.id == chosen));
    // What has become of the current clone generation's host binding — see
    // `CloneHostState`'s own doc for the four states and why
    // a bool cannot stand in for them. Reseeded to `Waiting` (or, for a
    // hostless clone, straight to `Unconfirmable`) every time a new
    // generation arrives, and otherwise updated only by the reseed effect
    // below (`resolve_clone_host`) and by an explicit host pick.
    let mut clone_host_state = use_signal(|| CloneHostState::Unconfirmable);
    // Whether this clone generation may still apply its source agent. Kept
    // separate from the host state because one helm catalog applies to every
    // host and explicit agent interaction must remain authoritative.
    let mut clone_agent_state = use_signal(|| CloneAgentState::Seeded);
    // Why this clone's own host is not (or is no longer) in play, when there
    // is something worth telling the user about it — read
    // straight off `clone_host_state` every render, never cached, so a
    // mismatch that resolves later (the registry catching up, an identity-
    // mismatch phase clearing) stops being reported the instant it stops
    // being true. `None` covers three unremarkable cases at once: no clone
    // is open, this generation is still `Waiting` on the registry (F1 says
    // nothing yet rather than guessing), and `Bound`/`UserTookOver`, where
    // there is nothing to explain.
    let clone_host_note = prefill
        .as_ref()
        .and_then(|prefill| match *clone_host_state.read() {
            CloneHostState::Unconfirmable if prefill.host.is_none() => Some(
                "the session you cloned predates host tracking, so its host could not be \
                 confirmed and the ordinary host default is used — check the host below",
            ),
            CloneHostState::Unconfirmable => Some(
                "the session you cloned reports a different installation now, so its host was \
                 not carried over here — check the host below",
            ),
            CloneHostState::Waiting | CloneHostState::Bound | CloneHostState::UserTookOver => None,
        });
    let selected = effective_create_host(&hosts, chosen_host(), open_host.as_ref());
    let destination_now = history_target(&hosts, selected);
    // Store the latest render's registry claim synchronously. Parent-derived
    // create_target has an intentional effect lag, while an old history
    // callback must already refuse a replacement visible in this render.
    let mut live_destination = use_signal(|| None::<CreateTarget>);
    if *live_destination.peek() != destination_now {
        live_destination.set(destination_now.clone());
    }
    let inherited_destination = initial_cwd.as_ref().and(destination_now.clone());
    let mut remembered_destination = use_signal(move || inherited_destination);
    let history_activation_attempts = use_signal(|| 0_u64);
    let remembered_destination_valid = remembered_destination_matches(
        remembered_destination.read().as_ref(),
        destination_now.as_ref(),
    );
    if remembered_destination_valid
        && error.peek().as_deref() == Some(REMEMBERED_DESTINATION_CHANGED)
    {
        error.set(None);
    }
    // Capture the immutable local id once for the destination link. Moving
    // the whole host list into that event handler would steal it from submit.
    let local_host_id = hosts.iter().find(|host| host.local).map(|host| host.id);
    let local_destination = history_target(&hosts, local_host_id);
    let browse_generation = use_signal(|| 0_u64);
    // A reply is useful only for the exact host connection and path that
    // asked for it. The generation rejects an older request; this tuple also
    // rejects a host or path edit that happened while one request was live.
    let browse_request = use_signal(|| None::<BrowseAuthority>);
    // Keep the reply's authority alongside its directory data. A browse is
    // a statement about one installation and one draft path, not a generic
    // folder picker result: a host retarget or a later folder choice must
    // make an already-arrived listing ineligible as well as rejecting a
    // listing that is still in flight.
    let browse_result = use_signal(|| None::<(BrowseAuthority, api::DirectoryBrowse)>);
    let browse_error = use_signal(|| None::<String>);
    // Count handler entry separately from successful activation. A result can
    // be removed by the next render, so the mounted browser regression needs
    // to distinguish "the stale click was refused" from "no callback ran".
    let mut browse_activation_attempts = use_signal(|| 0_u64);
    // Completion is distinct from rendering: rejected stale replies must not
    // become visible, but race tests still need to know the async task read
    // and checked the released response before continuing to another leg.
    let browse_reply_completions = use_signal(|| 0_u64);
    // The component receives a new host snapshot on each registry update,
    // but the async browse task outlives that render. This signal gives its
    // completion check the connection the form sees *now*, rather than the
    // token copied into the request it is trying to validate.
    let mut live_browse_connection = use_signal(|| None::<u64>);
    use_effect(use_reactive(
        &(hosts.clone(), selected),
        move |(hosts, selected)| {
            live_browse_connection.set(selected.and_then(|host| connection_claim(&hosts, host)));
        },
    ));
    // History is shared helm state. Its revision may change because another
    // browser launched a session, so its resource must react both to the
    // live destination and to the feed rather than to this render's host
    // snapshot.
    let history_revision = use_signal(|| 0_u64);
    let mut history_feed = history_revision;
    use_feed_reader(move || {
        history_feed.with_mut(|revision| *revision = revision.wrapping_add(1));
    });
    let history_base = base.clone();
    let history_target_signal = create_target;
    let launch_history = use_resource(move || {
        let base = history_base.clone();
        // This read is intentionally a dependency even though its numeric
        // value is not sent to the helm: it fetches a successful create in a
        // different client without disturbing the offered snapshot or this
        // dialog's draft. A later deliberate choice decides whether to
        // promote that fetched history.
        let _revision = history_revision();
        let target = history_target_signal();
        async move {
            let answer = match target.as_ref() {
                Some(target) => api::fetch_launch_history(&base, target.host).await,
                None => Ok(api::LaunchHistory::default()),
            };
            (target, answer)
        }
    });
    // A feed update is fetched immediately, but it is not allowed to rewrite
    // the suggestion surface beneath an open draft. The offered snapshot is
    // promoted only by a deliberate picker/search transition below. A new
    // destination is the exception: its old suggestions lose authority at
    // once and remain blank until a reply for the new installation arrives.
    let mut offered_history = use_signal(|| None::<(CreateTarget, api::LaunchHistory)>);
    let history_for_offer = launch_history
        .read()
        .as_ref()
        .and_then(|(target, result)| target.clone().zip(result.as_ref().ok().cloned()));
    // Feed replies update this handoff immediately, while `offered_history`
    // remains stable until a deliberate choice calls the shared promoter.
    let mut fetched_history = use_signal(|| None::<(CreateTarget, api::LaunchHistory)>);
    // This revision is deliberately exposed on the mounted dialog for the
    // browser contract tests. Offered history must remain unchanged when this
    // advances; the attribute lets those tests prove the component consumed a
    // particular fresh reply before they inspect that frozen offer.
    let mut fetched_history_revision = use_signal(|| 0_u64);
    if *fetched_history.peek() != history_for_offer {
        fetched_history.set(history_for_offer.clone());
        fetched_history_revision.with_mut(|revision| *revision = revision.wrapping_add(1));
    }
    // Event handlers are independently owned `FnMut` closures. Give each
    // deliberate-promotion boundary its own snapshot handle rather than
    // letting a search handler consume the value an ordinary recent needs.
    let history_for_search = history_for_offer.clone();
    let history_for_recents = history_for_offer.clone();
    let offer_target = create_target();
    if offered_history
        .peek()
        .as_ref()
        .is_some_and(|(target, _)| Some(target) != offer_target.as_ref())
    {
        offered_history.set(None);
    }
    if offered_history.peek().is_none()
        && let Some((target, history)) = history_for_offer.as_ref()
        && Some(target) == offer_target.as_ref()
    {
        offered_history.set(Some((target.clone(), history.clone())));
    }
    // This form's current intended create, if one has been submitted yet
    // (PLAN_M3.md item 6), together with the BINDING it was minted for.
    // Minted at first submit, reused by every later submit of the same
    // intent, and superseded the moment any part of that binding changes.
    let mut intent_key = use_signal(|| None::<(String, IntentBinding)>);
    let busy = ops.busy();

    // The mount and every structured/legacy surface change need the same
    // focus handoff. Reading only the surface means ordinary rerenders never
    // steal focus while someone is editing a field.
    use_effect(move || {
        let _surface = creation_surface();
        focus_composer_surface();
    });

    // Mounting this component is the create surface's closed-to-open
    // transition. An explicit request is allowed through a latched build skew
    // and coalesces with any read the page-owned surface already has in flight.
    use_effect(move || catalog.request(Trigger::Explicit));

    // Host binding and profile seeding now have different lifecycles. A host
    // installation change rotates the idempotency key and any host-specific
    // refusal, but the explicit profile choice survives because every host
    // consumes the same helm catalog. The remembered default is consumed once
    // for the dialog, whatever its first answer was, so a later feed refresh
    // cannot move the selection under someone filling in the form.
    let mut bound_target = use_signal(|| None::<CreateTarget>);
    let mut seeded_for = use_signal(|| false);
    // Which prefill GENERATION (`CreatePrefill`) this form has already
    // applied. Compared by generation rather than by mere presence in the
    // effect below, because `prefill` stays populated at its latest
    // generation for as long as this form is open, so presence alone would
    // reseed on every unrelated rerun of that effect (a host reconnect, a
    // catalog refresh) for as long as a clone is on screen.
    let mut prefill_applied = use_signal(|| None::<u64>);
    // Cloned rather than borrowed into the effect below: `hosts` is this
    // component's own prop (not a `Signal`, so it cannot be `Copy`-captured
    // the way the surrounding signals are), and the render body further
    // down needs its own, unmoved copy for the selector and the agent
    // picker.
    let hosts_for_reseed = hosts.clone();
    use_effect(use_reactive(
        (&prefill.as_ref().map(|prefill| prefill.generation),),
        move |_| {
            let hosts = &hosts_for_reseed;
            let target = create_target();
            let read = catalog.catalog.read();
            let previous = bound_target.peek().clone();

            // Applied BEFORE the host and agent resolutions below, and in the
            // SAME effect invocation rather than a separate one: a clone
            // aimed at a different host is itself what moves
            // `chosen_host`, and these fields must seed exactly once
            // whether or not the host also moves as a result.
            if let Some(prefill) = prefill
                .as_ref()
                .filter(|prefill| Some(prefill.generation) != *prefill_applied.peek())
            {
                prefill_applied.set(Some(prefill.generation));
                // A clone generation replaces the destination as a unit.
                // Invalidate before reseeding so an A→B→A clone sequence
                // cannot make an old A listing current again by restoring
                // matching host and folder text later in this effect.
                invalidate_directory_browse(
                    browse_generation,
                    browse_request,
                    browse_result,
                    browse_error,
                );
                reseed_cloned_field(&mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &prefill.cwd);
                // Clone owns a separate installation-reconciliation contract;
                // a new clone generation replaces any earlier history choice.
                remembered_destination.set(None);
                reseed_cloned_field(
                    &mut title,
                    &mut title_raw_seed,
                    &mut title_edited,
                    &prefill.title,
                );
                // Every clone generation establishes the WHOLE form state,
                // the dormant command field included — see `CreatePrefill::
                // invocation`'s own doc for why a profile-backed clone
                // still needs this written.
                reseed_cloned_field(
                    &mut invocation,
                    &mut invocation_raw_seed,
                    &mut invocation_edited,
                    &prefill.invocation,
                );
                if let Some(launch) = &prefill.launch {
                    // A structured snapshot is the source's explicit
                    // request, whereas its invocation is only the compiler's
                    // result. Preserve the former exactly for clone.
                    creation_surface.set(CreationSurface::Structured);
                    structured_harness.set(Some(launch.harness));
                    structured_model_raw_seed.set(launch.model.clone());
                    structured_model_edited.set(false);
                    structured_model.set(launch.model.clone());
                    // A restored model must establish ownership just like a
                    // freshly typed one. Known catalog ids override this
                    // fallback during harness reconciliation; an unknown id
                    // needs the source harness so it cannot leak across one.
                    custom_model_harness.set(launch.model.as_ref().map(|_| launch.harness));
                    structured_effort.set(launch.effort);
                    structured_permissions.set(launch.permissions);
                } else {
                    creation_surface.set(CreationSurface::Legacy);
                    structured_harness.set(None);
                    structured_model_raw_seed.set(None);
                    structured_model_edited.set(false);
                    structured_model.set(None);
                    custom_model_harness.set(None);
                    structured_effort.set(None);
                    structured_permissions.set(None);
                }
                // A prefill is as fresh an intent as any manual edit —
                // see the field `oninput` handlers below for both edges
                // of the "the key describes what was submitted" rule
                // this keeps.
                intent_key.set(None);
                // A refusal from before this clone described whatever
                // the form held then, which this prefill has just
                // replaced wholesale.
                error.set(None);
                // Compatibility prose belongs to the previous whole draft
                // too. A mounted second clone must not inherit a warning
                // about values this generation has already replaced.
                composer_reset_reason.set(None);

                // item2-review2.md F2: every new generation starts its OWN
                // host and agent decisions from a clean slate, cleared BEFORE any
                // attempt to resolve it — otherwise a clone whose own host
                // cannot be confirmed (rejected below, or hostless) could
                // silently inherit whatever a PREVIOUS generation (or an
                // earlier manual pick) had left in these three signals.
                chosen_host.set(None);
                chosen_profile.set(None);
                clone_agent_state.set(CloneAgentState::Waiting);
                clone_host_state.set(match prefill.host {
                    None => CloneHostState::Unconfirmable, // F4: nothing to resolve safely
                    Some(_) => CloneHostState::Waiting,
                });
            }

            // Resolve (or re-resolve) THIS generation's own host binding and
            // one-shot agent seed. Deliberately NOT gated on the generation transition
            // above — it runs on every pass this effect fires, reading
            // whatever `clone_host_state` currently holds, which is what
            // makes both F1's retry (once the registry answers a clone that
            // opened before it did) and F3's withdrawal (the instant a
            // `Bound` installation stops matching) possible: `prefill_applied`
            // above is a one-shot latch and will never route this
            // generation through the reseed branch a second time, so the
            // binding needs a path that keeps trying independently of it.
            if let Some(prefill) = prefill.as_ref() {
                let (next_state, action) = resolve_clone_host(
                    *clone_host_state.peek(),
                    prefill.host,
                    &prefill.host_identity,
                    hosts_loaded,
                    hosts,
                    prefill.host.is_some() && chosen_host.peek().as_ref() == prefill.host.as_ref(),
                );
                clone_host_state.set(next_state);
                match action {
                    CloneHostAction::Hold => {}
                    CloneHostAction::Bind(target) => {
                        chosen_host.set(Some(target.host));
                    }
                    CloneHostAction::Withdraw => {
                        chosen_host.set(None);
                    }
                }

                let offered = match read.answer() {
                    CatalogLookup::Known { catalog, .. } => Some(catalog),
                    CatalogLookup::Pending | CatalogLookup::Failed(_) => None,
                };
                let (next_agent_state, choice) =
                    resolve_clone_agent(*clone_agent_state.peek(), &prefill.agent, offered);
                clone_agent_state.set(next_agent_state);
                if let Some(choice) = choice {
                    chosen_profile.set(Some(choice));
                }
            }
            // An unconfirmable clone host leaves the selector at its ordinary
            // default. Agent seeding above is deliberately unaffected: the
            // source profile belongs to the helm, not to that installation.

            if previous != target {
                bound_target.set(target.clone());
                // Equality already compares the row and installation
                // fingerprint, so every target change here is a new intent.
                intent_key.set(None);
                // A refusal belongs to the host it came from. Left standing,
                // host A's refusal could be shown for host B.
                error.set(None);
            }
            if *seeded_for.peek() {
                return;
            }
            let CatalogLookup::Known { catalog: held, .. } = read.answer() else {
                return;
            };
            seeded_for.set(true);
            // The user (or a clone prefill, applied above) may have
            // answered while the read was in flight; an answer outranks a
            // default.
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
        },
    ));

    // What the picker may offer: the shared helm catalog once its read lands.
    let catalog_read = catalog.catalog.read();
    let held = catalog_read.answer();
    let offered = match &held {
        CatalogLookup::Known { catalog, .. } => Some(*catalog),
        _ => None,
    };
    let seeded = *seeded_for.read();
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
    // Profile mode is display-only: resolve the selected definition from the
    // current catalog, while custom mode keeps rendering the signal that
    // carries the clone seed and any user edits.
    let displayed_invocation = match (&agent.choice, offered) {
        (Some(AgentChoice::Profile(id)), Some(catalog)) => catalog
            .profiles
            .iter()
            .find(|profile| profile.id == *id)
            .map(|profile| display_peer(&profile.invocation))
            .unwrap_or_default(),
        _ => invocation.read().clone(),
    };

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
        let offered = match read.answer() {
            CatalogLookup::Known { catalog, .. } => Some(catalog),
            _ => None,
        };
        let seeded = *seeded_for.peek();
        resolve_agent(chosen_profile.peek().as_ref(), offered, seeded).choice
    };

    let catalog_models = launch_catalog
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let current_history_target = create_target();
    let recent_history = offered_history
        .read()
        .as_ref()
        .and_then(|(target, history)| {
            (Some(target) == current_history_target.as_ref()
                && Some(target) == destination_now.as_ref())
            .then_some(history)
        })
        .cloned()
        .unwrap_or_default();
    // Rank only combinations compatible with every explicit choice. Activating
    // one still replaces the whole draft; filtering decides which historical
    // combinations are offered, not which fields activation may write.
    let recent_filter = crate::launch_composer::ComposerFilter {
        harness: structured_harness(),
        model: structured_model(),
        effort: structured_effort(),
        permissions: structured_permissions(),
    };
    let recent_launches =
        crate::launch_composer::matching_recents(&recent_history, &recent_filter, Some(&cwd()))
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
    let selected_host_label = selected
        .and_then(|id| hosts.iter().find(|host| host.id == id))
        .map(HostOption::label)
        .unwrap_or_else(|| "unavailable host".to_string());
    // The host selector still shows unavailable destinations so a user can
    // see what changed, but an agent choice must not make Launch look ready
    // when the helm already knows that destination cannot accept it.
    let selected_host_available = selected.is_some_and(|id| {
        hosts
            .iter()
            .find(|host| host.id == id)
            .is_some_and(|host| host.phase.is_none())
    });
    let structured_efforts = structured_harness()
        .map(|harness| {
            crate::launch_composer::compatible_efforts(
                harness,
                structured_model().as_deref(),
                &catalog_models,
            )
        })
        .unwrap_or_default();
    // A stored structured snapshot is provenance, rather than a promise that
    // a later release still supports every combination it named. Keep it
    // visible for clone, but refuse to turn an incompatible known choice into
    // a different launch by guessing a replacement.
    let structured_choice_error = structured_harness().and_then(|harness| {
        let selection = LaunchSelection {
            harness,
            model: structured_model(),
            effort: structured_effort(),
            permissions: structured_permissions(),
        };
        if harness == LaunchHarness::OpenCode && selection.model.is_none() {
            Some("choose an OpenCode model before launching")
        } else {
            (!crate::launch_composer::selection_is_compatible(&selection, &catalog_models)).then_some(
                "this saved choice is no longer supported by the current catalog; choose a compatible model or effort",
            )
        }
    });
    // Peer-owned values remain separate directional runs. The launch context
    // isolates its host and folder, while the summary isolates its model, so a
    // strong RTL value cannot reorder the punctuation around another value.
    //
    // The harness is named only on the structured surface. Switching to
    // "other / command" deliberately keeps the structured draft (so a trip
    // through Other and back loses nothing), but a legacy launch runs the
    // chosen profile or command, never that harness — so the button must not
    // promise "Codex" while the click would launch something else.
    let launch_harness = if *creation_surface.read() == CreationSurface::Structured {
        structured_harness().map(|harness| format!("{harness:?}"))
    } else {
        None
    };
    let summary_folder = display_peer(&cwd());
    let summary_model = structured_model()
        .map(|model| display_peer(&model))
        .unwrap_or_else(|| "default".to_string());
    let summary_effort = structured_effort().map(effort_value).unwrap_or("default");
    let catalog_for_submit = catalog_models.clone();
    let catalog_for_harness = catalog_models.clone();
    let catalog_for_search = catalog_models.clone();
    let catalog_for_custom_model = catalog_models.clone();
    // Pointer activation and Enter both apply exactly the same saved draft.
    // Enter deliberately submits only AFTER this callback returns: the form's
    // submit handler owns the intent key and operation lock, so it must remain
    // the one path that can mint a key for a launch.
    let apply_recent = Callback::<api::LaunchHistoryEntry, bool>::new({
        let catalog = catalog_models.clone();
        let history = history_for_recents.clone();
        let history_target = current_history_target.clone();
        move |entry: api::LaunchHistoryEntry| {
            if !draft_transition_allowed(ops) {
                return false;
            }
            if !admit_history_destination(
                history_target.clone(),
                live_destination,
                remembered_destination,
                history_activation_attempts,
            ) {
                return false;
            }
            promote_history_snapshot(offered_history, create_target(), history.clone());
            let selection = crate::launch_composer::select_recent(&entry);
            structured_harness.set(Some(selection.harness));
            structured_model_raw_seed.set(selection.model.clone());
            structured_model_edited.set(false);
            structured_model.set(selection.model);
            custom_model_harness.set(entry.selection.model.as_ref().and_then(|model| {
                (!catalog.iter().any(|candidate| candidate.id == *model))
                    .then_some(entry.selection.harness)
            }));
            structured_effort.set(selection.effort);
            structured_permissions.set(selection.permissions);
            invalidate_directory_browse(
                browse_generation,
                browse_request,
                browse_result,
                browse_error,
            );
            reseed_cloned_field(&mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &entry.cwd);
            composer_reset_reason.set(None);
            intent_key.set(None);
            true
        }
    });
    let search_result_groups =
        crate::launch_composer::grouped_search_results(crate::launch_composer::search_results(
            &recent_history,
            &catalog_models,
            &composer_search(),
        ));
    let search_results_for_keys = search_result_groups
        .iter()
        .flat_map(|(_, results)| results.iter().cloned())
        .collect::<Vec<_>>();
    let browse_base = base.clone();
    // This snapshot is used only to construct the outbound request. The
    // reply is checked against `browse_target`, which stays live while the
    // request is in flight.
    let browse_hosts = hosts.clone();
    let browse_target = create_target;
    let browse_cwd = cwd;
    let browse_cwd_raw_seed = cwd_raw_seed;
    let browse_cwd_edited = cwd_edited;
    let browse_base_for_folder = browse_base.clone();
    let browse_hosts_for_folder = browse_hosts.clone();
    let hosts_for_destination_choice = hosts.clone();
    // The host selector and its reconciliation notes are built ONCE and placed
    // by whichever surface renders: inside the structured destination block,
    // or under the legacy surface's own "host" label. One Element cloned into
    // two mutually exclusive branches keeps the handler, the option marking,
    // and the four notes from drifting apart the way two copies would.
    //
    // The select is disabled for the whole round trip, exactly like the text
    // fields: the key is bound to the target, and a selection changing between
    // minting and sending would publish a key that belongs to a different
    // machine. Its value is empty only before the first hosts read lands (a
    // live helm always has its local row); the submit handler refuses in that
    // window rather than sending a hostless create.
    let host_select = rsx! {
        select {
            class: "create-session-host",
            aria_label: "host",
            disabled: busy,
            value: selected.map(|id| id.to_string()).unwrap_or_default(),
            onchange: move |evt| {
                if !draft_transition_allowed(ops) {
                    return;
                }
                let next_host = evt.value().parse::<HostId>().ok();
                chosen_host.set(next_host);
                // A queued history callback can run before rerender.
                // Revoke the old destination in this same event turn.
                live_destination.set(history_target(&hosts_for_destination_choice, next_host));
                remembered_destination.set(None);
                invalidate_directory_browse(
                    browse_generation, browse_request, browse_result, browse_error,
                );
                // The agent choice deliberately survives: every host
                // consumes the same helm catalog.
                // And it takes this generation's clone-derived
                // binding off automatic handling for good
                // (`CloneHostState::UserTookOver`): the user is now
                // driving host selection by hand, so a later
                // retarget of the CLONE's own row must not pull the
                // rug out from under a choice the clone had nothing
                // to do with anymore.
                clone_host_state.set(CloneHostState::UserTookOver);
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
    };
    // The reconciliation, said out loud. A chosen host leaving the registry
    // moves the effective target, and the one thing that must not happen is
    // that move being invisible — a selector showing host A while the body
    // carries host B is a create on a machine nobody picked. The key is
    // re-minted for the new target by the submit path's own binding check.
    //
    // The clone-specific reconciliation sits in the same voice and the same
    // slot: the row this form was cloned from could not be confirmed as the
    // install it was cloned from (mismatched, or predating host tracking
    // entirely — see `clone_host_note`'s own doc), so its host was not
    // carried over, and the selector shows its ordinary default instead. Says
    // nothing while a hostful clone is still `Waiting` on the registry (F1) —
    // that is not yet a fact worth reporting.
    let host_notes = rsx! {
        if choice_vanished {
            div { class: "create-session-host-note",
                "the host you picked is no longer registered, so this create would go to the \
                 one selected now"
            }
        }
        if selected.is_some() && !selected_host_available {
            div { class: "create-session-host-note",
                "the selected host is unavailable; choose a connected host before launching"
            }
        }
        if !remembered_destination_valid {
            div { class: "create-session-host-note",
                "{REMEMBERED_DESTINATION_CHANGED}"
            }
        }
        if let Some(note) = clone_host_note {
            div { class: "create-session-host-note", "{note}" }
        }
    };
    // The two destination resets, built once for the same reason as the host
    // select: both surfaces need them (a clone from a remote host is what makes
    // "local home" necessary, on the legacy surface as much as the structured
    // one), and one Element keeps the two handlers from drifting apart.
    // "home" reseeds only the folder; "local home" also moves the host to the
    // local machine and takes the clone's host binding off automatic handling.
    let destination_resets = rsx! {
        button { r#type: "button", disabled: busy,
            aria_label: "reset folder to home",
            onclick: move |_| {
                if !draft_transition_allowed(ops) { return; }
                remembered_destination.set(None);
                promote_fetched_history_snapshot(
                    offered_history, create_target, fetched_history,
                );
                invalidate_directory_browse(
                    browse_generation, browse_request, browse_result, browse_error,
                );
                reseed_cloned_field(&mut cwd, &mut cwd_raw_seed, &mut cwd_edited, "~");
                intent_key.set(None);
            },
            "home"
        }
        span { class: "launch-composer-folder-links-separator", aria_hidden: "true", "·" }
        button { r#type: "button", disabled: busy,
            aria_label: "reset destination to local home",
            onclick: move |_| {
                if !draft_transition_allowed(ops) { return; }
                // This is an explicit local choice, unlike the ordinary
                // default that may follow an open remote session.
                chosen_host.set(local_host_id);
                live_destination.set(local_destination.clone());
                remembered_destination.set(None);
                promote_fetched_history_snapshot(
                    offered_history, create_target, fetched_history,
                );
                clone_host_state.set(CloneHostState::UserTookOver);
                invalidate_directory_browse(
                    browse_generation, browse_request, browse_result, browse_error,
                );
                reseed_cloned_field(&mut cwd, &mut cwd_raw_seed, &mut cwd_edited, "~");
                intent_key.set(None);
            },
            "local home"
        }
    };
    rsx! {
        div {
            class: "launch-composer-backdrop",
            role: "presentation",
            onclick: move |_| {
                // This is a handler-time decision. The rendered `busy` value
                // can be one turn stale immediately after submit, when
                // unmounting would cancel the create future and strand its
                // page-operation claim.
                if !ops.busy_now() {
                    on_cancel.call(());
                }
            },
        form {
            class: "create-session-form",
            // These are diagnostic state, not an authority channel. Keeping
            // the live connection and activation count on the mounted form
            // gives browser tests a synchronous observation point for races
            // that deliberately keep the visible result unchanged.
            "data-history-fetched-revision": "{fetched_history_revision}",
            "data-browse-live-connection": "{live_browse_connection().unwrap_or_default()}",
            "data-browse-activation-attempts": "{browse_activation_attempts}",
            "data-browse-reply-completions": "{browse_reply_completions}",
            "data-history-activation-attempts": "{history_activation_attempts}",
            "data-remembered-destination-valid": "{remembered_destination_valid}",
            role: "dialog",
            aria_modal: "true",
            aria_label: "launch a session",
            onmounted: move |_| {
                install_composer_focus_trap();
                focus_composer_surface();
            },
            onclick: move |evt| {
                // A click that leaves the search surface must make its
                // hidden results ineligible before another control handles
                // a following Enter. The search wrapper stops its own
                // events, so typing and choosing inside it keep the listbox.
                composer_search_open.set(false);
                evt.stop_propagation();
            },
            onkeydown: move |evt| {
                if evt.key() == Key::Escape && !ops.busy_now() {
                    on_cancel.call(());
                }
            },
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
                let Some(op_guard) = ops.claim_guard() else {
                    return;
                };
                // No agent, no create. "Nothing is selected" is a real state
                // rather than a gap to be filled — a profile that was chosen
                // or remembered and has since been deleted leaves the dialog
                // waiting for an answer, and the command field it would
                // otherwise fall back to still holds whatever was typed into
                // it earlier. Launching that would run something nobody
                // picked while the note beside it said nothing was selected.
                // Frozen HERE, from what was just resolved, and not touched
                // again: the minting await below can span a deletion or
                // another client's remembered-default write, and re-resolving
                // across it would let the request's MODE differ from the one
                // the button was pressed on. A profile that goes away in that
                // window is refused by the supervisor, by name.
                let launch = if *creation_surface.peek() == CreationSurface::Structured {
                    let Some(harness) = *structured_harness.peek() else {
                        error.set(Some("choose a structured harness before launching".to_string()));
                        ops.release();
                        return;
                    };
                    let selection = LaunchSelection {
                        harness,
                        model: structured_model.peek().clone(),
                        effort: *structured_effort.peek(),
                        permissions: *structured_permissions.peek(),
                    };
                    if harness == LaunchHarness::OpenCode && selection.model.is_none() {
                        error.set(Some("choose an OpenCode model before launching".to_string()));
                        ops.release();
                        return;
                    }
                    if !crate::launch_composer::selection_is_compatible(
                        &selection,
                        &catalog_for_submit,
                    ) {
                        error.set(Some(
                            "this saved choice is no longer supported by the current catalog; choose a compatible model or effort".to_string(),
                        ));
                        ops.release();
                        return;
                    }
                    LaunchIntent::Structured(selection)
                } else {
                    let Some(choice) = resolve_now() else {
                        error.set(Some("no agent is selected for this create — choose a profile, or choose \"custom command\" to run the command below".to_string()));
                        ops.release();
                        return;
                    };
                    match choice {
                    // The RAW bytes while untouched, not the escaped display
                    // the field shows — `profiles::submitted_field` is the
                    // same read-back rule the profile editor uses for its
                    // own peer-relayed fields (item2-review2.md F5).
                    AgentChoice::Command => LaunchIntent::Command(submitted_field(
                        &invocation.peek(),
                        *invocation_edited.peek(),
                        invocation_raw_seed.peek().as_deref(),
                    )),
                        AgentChoice::Profile(id) => LaunchIntent::Profile(id),
                    }
                };
                // The HOST is derived here too, from the live signal — never
                // from what the last render computed. The same one-turn window
                // the agent has: changing the selector and pressing create in
                // one turn reaches this handler before any re-render, and a
                // captured host would send the create to the PREVIOUS machine
                // while the selector on screen names another.
                let target_now = create_target.peek().clone();
                let selected_now =
                    effective_create_host(&hosts, chosen_host.peek().to_owned(), open_host.as_ref());
                if !remembered_destination_matches(
                    remembered_destination.peek().as_ref(),
                    live_destination.peek().as_ref(),
                ) {
                    error.set(Some(REMEMBERED_DESTINATION_CHANGED.to_string()));
                    return;
                }
                // The host target must have caught up with the selector before
                // the request can bind its idempotency and connection claims.
                if !target_matches_selection(selected_now, &hosts, target_now.as_ref()) {
                    error.set(Some(
                        "the target host changed while this create was being submitted, so                          nothing was sent — check the agent and press create again"
                            .to_string(),
                    ));
                    ops.release();
                    return;
                }
                if !selected_now.is_some_and(|id| {
                    hosts
                        .iter()
                        .find(|host| host.id == id)
                        .is_some_and(|host| host.phase.is_none())
                }) {
                    error.set(Some(
                        "the selected host is unavailable, so this create was not sent — choose a connected host"
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
                let Some(binding) = IntentBinding::of(
                    selected_now,
                    &hosts,
                    submitted_field(&cwd(), cwd_edited(), cwd_raw_seed.peek().as_deref()),
                    launch,
                    submitted_field(&title(), title_edited(), title_raw_seed.peek().as_deref()),
                ) else {
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
                // The target row's install identity as this form knew it at
                // submit — resolved here, outside the spawn, because
                // `binding.host` is frozen (the mint loop re-reads text
                // fields only) and the reply below carries no host facts to
                // resolve it from. It backfills the created `Session` so
                // the selection this create becomes carries the same
                // install claim a listing row would have carried.
                let created_host_identity = hosts
                    .iter()
                    .find(|host| host.id == binding.host)
                    .map(|host| host.identity.clone());
                // The connection this create is prepared against, read from
                // the same hosts snapshot the target match above just
                // vouched for: the helm refuses the create if the host has
                // been retargeted or adopted onto another install by the
                // time it routes, which keeps any launch intent from reaching
                // a successor installation the form never selected. `None` —
                // a vanished row, or one that has
                // never connected — means no claim (see `connection_claim`).
                let expected_incarnation = connection_claim(&hosts, binding.host);
                error.set(None);
                let catalog_for_recheck = catalog_for_submit.clone();
                spawn(async move {
                    // Own the release through every await. If navigation or a
                    // parent state change drops this component, dropping this
                    // future releases the shared mutation gate as well.
                    let _op_guard = op_guard;
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
                        // Profile/default changes are intentionally frozen at
                        // the press: a catalog refresh is not a new user
                        // choice. Structured controls are different. Every
                        // visible harness/model/effort/permission click is a
                        // deliberate edit, and it may have been queued ahead
                        // of the render that disables controls. Re-read that
                        // complete declarative intent so the next key binds
                        // exactly what the person now sees.
                        if matches!(binding.agent, LaunchIntent::Structured(_)) {
                            let Some(harness) = *structured_harness.peek() else {
                                error.set(Some(
                                    "the structured launch choice changed while its idempotency key was being generated; choose a harness and press launch again".to_string(),
                                ));
                                ops.release();
                                return;
                            };
                            let selection = LaunchSelection {
                                harness,
                                model: structured_model.peek().clone(),
                                effort: *structured_effort.peek(),
                                permissions: *structured_permissions.peek(),
                            };
                            if *creation_surface.peek() != CreationSurface::Structured
                                || !crate::launch_composer::selection_is_compatible(
                                    &selection,
                                    &catalog_for_recheck,
                                )
                            {
                                error.set(Some(
                                    "the structured launch choice changed while its idempotency key was being generated; review it and press launch again".to_string(),
                                ));
                                ops.release();
                                return;
                            }
                            binding.agent = LaunchIntent::Structured(selection);
                        }
                        binding = IntentBinding {
                            cwd: submitted_field(
                                &cwd.peek(),
                                *cwd_edited.peek(),
                                cwd_raw_seed.peek().as_deref(),
                            ),
                            title: submitted_field(
                                &title.peek(),
                                *title_edited.peek(),
                                title_raw_seed.peek().as_deref(),
                            ),
                            ..binding
                        };
                    };
                    // Key, fields, host AND creation mode all travel from ONE
                    // value, so there is no arrangement of edits or reads in
                    // which the body describes a different intent than the
                    // key claims — including the mode itself, which the
                    // supervisor folds into its own idempotency fingerprint
                    // precisely so a retried create cannot flip it.
                    // Recheck after key minting, including retries that already
                    // had a key. A new target must not legitimize an old path.
                    if !remembered_destination_matches(
                        remembered_destination.peek().as_ref(),
                        live_destination.peek().as_ref(),
                    ) {
                        error.set(Some(REMEMBERED_DESTINATION_CHANGED.to_string()));
                        intent_key.set(None);
                        return;
                    }
                    let agent = match &bound.agent {
                        LaunchIntent::Command(invocation) => CreateAgent::Command(invocation),
                        LaunchIntent::Profile(id) => CreateAgent::Profile(id),
                        LaunchIntent::Structured(selection) => CreateAgent::Structured(selection),
                    };
                    match create_session(
                        &base,
                        &bound.cwd,
                        agent,
                        &bound.title,
                        &key,
                        Some(bound.host),
                        expected_incarnation,
                    )
                    .await
                    {
                        Ok(session) => {
                            // A profile-backed create changes the helm's
                            // remembered default. Drop the old paired answer
                            // before this form closes so an immediate reopen
                            // stays pending until an authoritative read lands;
                            // writing the submitted id locally would pretend
                            // the helm's best-effort preference write is known
                            // to have succeeded.
                            if matches!(&bound.agent, LaunchIntent::Profile(_)) {
                                catalog.invalidate();
                                catalog.request(Trigger::Explicit);
                            }
                            // Released before navigating: `on_created`
                            // unmounts this component, and a token released
                            // afterwards would be released by a task nobody
                            // is left to run.
                            ops.release();
                            on_created.call(enrich_created_session(
                                session,
                                bound.host,
                                created_host_identity,
                            ));
                        }
                        Err(e) => {
                            // Gated on the target this request was DISPATCHED
                            // for still being the one on screen: a refusal
                            // naming host A must not land under a form that
                            // has since been re-pointed at host B, where it
                            // would describe a machine the user is not looking
                            // at and may not even be true.
                            if create_target.peek().as_ref() == target_now.as_ref() {
                                let (stale, prose) = api::precondition_of(&e);
                                if stale {
                                    // The world moved between preparing this
                                    // create and routing it — the id now
                                    // reaches another install, where the
                                    // connection claim no longer describes the
                                    // selected installation. The key goes with it (a
                                    // retry must be a NEW intent, not a replay
                                    // aimed at a machine that never saw the
                                    // first) and the catalog is re-read, which
                                    // is what supersedes this message.
                                    intent_key.set(None);
                                }
                                // The key otherwise deliberately SURVIVES a
                                // failure: a failure whose cause was an
                                // ambiguous transport error may have created
                                // a session the user cannot see, and
                                // resubmitting unchanged must reach that same
                                // session rather than launch a second agent.
                                // A user who instead fixes the form gets a new
                                // key, because the binding no longer matches.
                                error.set(Some(prose));
                            }
                            ops.release();
                        }
                    }
                });
            },
            div { class: "launch-composer-topbar",
                span { class: "launch-composer-eyebrow", "new session" }
            }
            // Launch leads because reaching it was the maintainer's complaint:
            // it now says exactly which harness and destination it will use.
            // Name is optional context rather than an action, so its two
            // surface-specific placements live with their destination fields
            // below; exactly one copy is mounted at a time.
            div { class: "launch-composer-actions",
                button {
                    r#type: "submit",
                    class: "btn btn-primary create-session-submit",
                    // `blocked` as well as this form's own flag: a create must
                    // not overlap a host mutation (see `ListView`'s operation
                    // gate), and a control that is inert for that window says so
                    // rather than silently dropping the click.
                    //
                    // Inert with no agent selected for a different reason: there
                    // is nothing to launch, and the handler refuses in words
                    // anyway (a `disabled` attribute is one render behind, so it
                    // is the visible half of that rule rather than the guard).
                    disabled: busy
                        || !selected_host_available
                        || !remembered_destination_valid
                        || (*creation_surface.read() == CreationSurface::Structured
                            && (structured_harness.read().is_none()
                                || structured_choice_error.is_some()))
                        || (*creation_surface.read() == CreationSurface::Legacy && agent.choice.is_none()),
                    "launch"
                    " "
                    span { class: "launch-composer-launch-context",
                        if let Some(harness) = &launch_harness {
                            "{harness} · "
                        }
                        span { class: "peer-value", dir: "ltr", "{selected_host_label}" }
                        " · "
                        span { class: "peer-value", dir: "ltr", "{summary_folder}" }
                    }
                }
                button {
                    r#type: "button",
                    class: "launch-composer-cancel",
                    disabled: busy,
                    onclick: move |_| {
                        // The disabled attribute updates after this event's
                        // synchronous submit claim. Recheck the shared lock
                        // here so a queued Cancel cannot unmount the future
                        // that owns an already accepted create.
                        if !ops.busy_now() {
                            on_cancel.call(());
                        }
                    },
                    "cancel"
                }
                if *creation_surface.read() == CreationSurface::Structured {
                    button {
                        r#type: "button",
                        class: "launch-composer-reset",
                        disabled: busy,
                        onclick: move |_| {
                            if !draft_transition_allowed(ops) {
                                return;
                            }
                            // Reset only the declarative launch choices. The
                            // host and folder are launch context, often
                            // supplied by the selected session, and clearing
                            // them would turn a quick correction into a new
                            // destination decision.
                            structured_harness.set(None);
                            structured_model_raw_seed.set(None);
                            structured_model_edited.set(false);
                            structured_model.set(None);
                            custom_model_harness.set(None);
                            structured_effort.set(None);
                            structured_permissions.set(None);
                            composer_reset_reason.set(None);
                            promote_fetched_history_snapshot(
                                offered_history, create_target, fetched_history,
                            );
                            composer_search.set(String::new());
                            composer_search_open.set(false);
                            intent_key.set(None);
                        },
                        "reset choices"
                    }
                }
            }
            if *creation_surface.read() == CreationSurface::Structured {
                div { class: "launch-composer-summary", aria_live: "polite",
                    "model: "
                    span { class: "peer-value", dir: "ltr", "{summary_model}" }
                    " · effort: {summary_effort} · permissions: "
                    // Matched on the signal, not on display text, and spelled
                    // out per variant so every permission reads in the same
                    // lowercase register as "default": a Debug fallback would
                    // capitalize a future variant next to these.
                    if structured_permissions() == Some(LaunchPermission::Yolo) {
                        span { class: "launch-composer-danger", "yolo" }
                    } else {
                        "default"
                    }
                }
            }
            if *creation_surface.read() == CreationSurface::Structured {
                div {
                    class: "launch-composer-search",
                    onclick: move |evt| evt.stop_propagation(),
                    input {
                        r#type: "search",
                        role: "combobox",
                        aria_label: "search folders, harnesses, and models",
                        aria_expanded: composer_search_open(),
                        aria_controls: "launch-composer-search-results",
                        aria_activedescendant: (composer_search_open() && !search_results_for_keys.is_empty())
                            .then(|| format!("launch-composer-search-option-{}", composer_search_index())),
                        placeholder: "search folders, harnesses, models…",
                        autocomplete: "off",
                        // Search includes literal host paths and model IDs;
                        // browser text correction would change the query's meaning.
                        autocorrect: "off",
                        autocapitalize: "none",
                        spellcheck: "false",
                        // The dialog opens in structured mode, so its
                        // searchable chooser is the first meaningful focus
                        // target rather than leaving keyboard users behind
                        // the newly mounted modal.
                        autofocus: true,
                        // Dioxus can retain an already-created browser node
                        // across the render that makes this dialog visible,
                        // and HTML only applies `autofocus` while inserting a
                        // node. Ask the mounted renderer as well so the
                        // keyboard handoff is real in WebKit and Chromium,
                        // not merely present in the serialized markup.
                        onmounted: move |element| {
                            let input = element.data();
                            spawn(async move {
                                let _ = input.set_focus(true).await;
                            });
                        },
                        value: "{composer_search}",
                        disabled: busy,
                        oninput: move |evt| {
                            promote_history_snapshot(
                                offered_history, create_target(), history_for_search.clone(),
                            );
                            composer_search.set(evt.value());
                            composer_search_open.set(true);
                            composer_search_index.set(0);
                        },
                        onkeydown: {
                            let catalog = catalog_for_search.clone();
                            let browse_base = browse_base.clone();
                            let browse_hosts = browse_hosts.clone();
                            let history_target = current_history_target.clone();
                            move |evt| {
                            match evt.key() {
                                // Escape belongs to search only while its
                                // result surface is open. Once that surface
                                // is already gone, let the dialog's handler
                                // receive the same key and dismiss the draft.
                                // Consuming both states strands keyboard
                                // users on an otherwise closed combobox.
                                Key::Escape if composer_search_open() => {
                                    evt.prevent_default();
                                    evt.stop_propagation();
                                    composer_search_open.set(false);
                                }
                                Key::ArrowDown if composer_search_open() && !search_results_for_keys.is_empty() => {
                                    evt.prevent_default();
                                    composer_search_index.set(
                                        (composer_search_index() + 1)
                                            % search_results_for_keys.len(),
                                    );
                                    scroll_composer_search_result(composer_search_index());
                                }
                                Key::ArrowUp if composer_search_open() && !search_results_for_keys.is_empty() => {
                                    evt.prevent_default();
                                    composer_search_index.set(
                                        (composer_search_index() + search_results_for_keys.len() - 1)
                                            % search_results_for_keys.len(),
                                    );
                                    scroll_composer_search_result(composer_search_index());
                                }
                                // While the combobox owns focus, Enter belongs to search even
                                // when the query has no matches. Otherwise a no-result query
                                // bubbles to the form and launches whatever stale selection the
                                // composer happened to hold.
                                Key::Enter if !evt.is_composing() => {
                                    evt.prevent_default();
                                    if !draft_transition_allowed(ops) {
                                        return;
                                    }
                                    if composer_search_open() && let Some(result) =
                                        search_results_for_keys.get(composer_search_index()).cloned()
                                    {
                                        // Every selection invalidates an old directory listing
                                        // before it changes the draft. BrowsePath installs its
                                        // replacement request below, so a predecessor cannot
                                        // win the race between these two UI transitions.
                                        invalidate_directory_browse(
                                            browse_generation, browse_request, browse_result, browse_error,
                                        );
                                        // Keep `result` as the action this key accepted. Promotion
                                        // changes later suggestions only; it must not substitute a
                                        // fresh matching result between key handling and draft apply.
                                        promote_fetched_history_snapshot(
                                            offered_history, create_target, fetched_history,
                                        );
                                        let browse_path = apply_composer_search_result(
                                            result,
                                            history_target.clone(),
                                            live_destination,
                                            remembered_destination,
                                            history_activation_attempts,
                                            cwd,
                                            cwd_raw_seed,
                                            cwd_edited,
                                            structured_harness,
                                            structured_model,
                                            structured_model_raw_seed,
                                            structured_model_edited,
                                            custom_model_harness,
                                            structured_effort,
                                            structured_permissions,
                                            composer_reset_reason,
                                            &catalog,
                                            intent_key,
                                        );
                                        if let Some(path) = browse_path {
                                            request_directory_browse(
                                                browse_base.clone(),
                                                selected,
                                                &browse_hosts,
                                                browse_target,
                                                path,
                                                browse_generation,
                                                browse_request, browse_result, browse_error, browse_reply_completions,
                                                live_browse_connection, cwd, cwd_raw_seed, cwd_edited,
                                            );
                                        }
                                        composer_search.set(String::new());
                                        composer_search_open.set(false);
                                        focus_composer_surface();
                                    }
                                }
                                _ => {}
                            }
                        }
                        },
                    }
                    if composer_search_open() && !search_result_groups.is_empty() {
                        div {
                            id: "launch-composer-search-results",
                            role: "listbox",
                            class: "launch-composer-search-results",
                            for (group_index, (group, results)) in search_result_groups.iter().cloned().enumerate() {
                                div {
                                    role: "group",
                                    aria_label: group.label(),
                                    class: if group == crate::launch_composer::ComposerSearchGroup::RecentSetups { "launch-composer-search-group launch-composer-search-recents" } else { "launch-composer-search-group" },
                                    div { class: "launch-composer-search-group-heading", "{group.label()}" }
                                    for (result_index, result) in results.into_iter().enumerate() {
                                        {
                                            let index = search_result_groups[..group_index]
                                                .iter()
                                                .map(|(_, prior)| prior.len())
                                                .sum::<usize>() + result_index;
                                            rsx! {
                                                button {
                                                    id: "launch-composer-search-option-{index}",
                                                    r#type: "button",
                                                    role: "option",
                                                    dir: "ltr",
                                                    aria_selected: composer_search_index() == index,
                                                    class: if matches!(result, crate::launch_composer::ComposerSearchResult::Recent(_)) {
                                                        if composer_search_index() == index { "launch-composer-search-recent selected" } else { "launch-composer-search-recent" }
                                                    } else if composer_search_index() == index { "selected" } else { "" },
                                                    title: match &result {
                                                        crate::launch_composer::ComposerSearchResult::Recent(entry) => format!(
                                                            "{} · {} · {}",
                                                            display_peer(&entry.cwd), selected_host_label,
                                                            display_peer(&crate::launch_composer::selection_summary(&entry.selection)),
                                                        ),
                                                        _ => String::new(),
                                                    },
                                                    // Search recents use two visual spans as ordinary
                                                    // recents do. Their accessible label repeats the
                                                    // complete title with the result kind, instead of
                                                    // losing the separator where those spans meet.
                                                    aria_label: match &result {
                                                        crate::launch_composer::ComposerSearchResult::Recent(entry) => format!(
                                                            "Recent setup: {} · {} · {}",
                                                            display_peer(&entry.cwd), selected_host_label,
                                                            display_peer(&crate::launch_composer::selection_summary(&entry.selection)),
                                                        ),
                                                        _ => String::new(),
                                                    },
                                                    onclick: {
                                                        let result = result.clone();
                                                        let catalog = catalog_for_search.clone();
                                                        let browse_base = browse_base.clone();
                                                        let browse_hosts = browse_hosts.clone();
                                                        let history_target = current_history_target.clone();
                                                        move |_| {
                                                            if !draft_transition_allowed(ops) {
                                                                return;
                                                            }
                                                            invalidate_directory_browse(
                                                                browse_generation, browse_request, browse_result, browse_error,
                                                            );
                                                            // The click owns the captured result. A
                                                            // newer history may refresh suggestions,
                                                            // never the result this click applies.
                                                            promote_fetched_history_snapshot(
                                                                offered_history, create_target, fetched_history,
                                                            );
                                                            let browse_path = apply_composer_search_result(
                                                                result.clone(), history_target.clone(),
                                                                live_destination, remembered_destination,
                                                                history_activation_attempts, cwd, cwd_raw_seed, cwd_edited,
                                                                structured_harness, structured_model,
                                                                structured_model_raw_seed, structured_model_edited,
                                                                custom_model_harness, structured_effort,
                                                                structured_permissions, composer_reset_reason,
                                                                &catalog, intent_key,
                                                            );
                                                            if let Some(path) = browse_path {
                                                                request_directory_browse(
                                                                    browse_base.clone(), selected, &browse_hosts,
                                                                    browse_target, path, browse_generation,
                                                                    browse_request, browse_result, browse_error, browse_reply_completions,
                                                                    live_browse_connection, cwd, cwd_raw_seed, cwd_edited,
                                                                );
                                                            }
                                                            composer_search.set(String::new());
                                                            composer_search_open.set(false);
                                                            focus_composer_surface();
                                                        }
                                                    },
                                                    match &result {
                                                        // Folder actions alter only cwd; a recent
                                                        // setup visibly names every choice it owns.
                                                        crate::launch_composer::ComposerSearchResult::UsePath(folder)
                                                        | crate::launch_composer::ComposerSearchResult::Folder(folder) => rsx! { "Use this path: {display_peer(folder)}" },
                                                        crate::launch_composer::ComposerSearchResult::BrowsePath(folder) => rsx! { "Browse this path: {display_peer(folder)}" },
                                                        crate::launch_composer::ComposerSearchResult::Harness(harness) => rsx! { "Harness: {harness:?}" },
                                                        crate::launch_composer::ComposerSearchResult::Model { id, harness } => rsx! { "Model: {display_peer(id)} ({harness:?})" },
                                                        crate::launch_composer::ComposerSearchResult::Recent(entry) => rsx! {
                                                            span { class: "launch-composer-search-recent-destination", "Recent setup: {display_peer(&entry.cwd)} · {selected_host_label}" }
                                                            span { class: "launch-composer-search-recent-selection", "{display_peer(&crate::launch_composer::selection_summary(&entry.selection))}" }
                                                        },
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
                // Absent, not merely empty, with no history to offer: a
                // first-time host or unmatched filter should not make the
                // composer advertise an empty action group. Nothing below
                // this point reserves space, so a later match grows the
                // layout rather than revealing a band that was secretly
                // already there.
                if !recent_launches.is_empty() {
                    div { class: "launch-composer-recents",
                            span { class: "launch-composer-section-label", "recent setups" }
                            div { class: "launch-composer-recent-slots",
                            for (entry, summary, before_permissions) in recent_launches.iter().take(3).map(|entry| (
                                entry,
                                crate::launch_composer::selection_summary(&entry.selection),
                                crate::launch_composer::selection_summary_before_permissions(&entry.selection),
                            )) {
                                button {
                                    r#type: "button",
                                    dir: "ltr",
                                    disabled: busy,
                                    title: "{display_peer(&entry.cwd)} · {selected_host_label} · {display_peer(&summary)}",
                                    // The visible row makes scanning cheaper, but
                                    // its title and accessible name retain every
                                    // saved value that the one-line layout may
                                    // truncate. The punctuated string also keeps
                                    // adjacent spans from running together for
                                    // assistive technology.
                                    aria_label: "{display_peer(&entry.cwd)} · {selected_host_label} · {display_peer(&summary)}",
                                    onclick: {
                                        let entry = entry.clone();
                                        move |_| {
                                            apply_recent.call(entry.clone());
                                        }
                                    },
                                    onkeydown: {
                                        let entry = entry.clone();
                                        move |evt| {
                                            // The busy guard lives in `apply_recent`
                                            // (its `false` return below); only the key
                                            // itself is filtered here.
                                            if evt.key() != Key::Enter || evt.is_composing() {
                                                return;
                                            }
                                            // Native button activation would emit a
                                            // synthetic click after Enter. Suppress it
                                            // so this row fills and submits once.
                                            evt.prevent_default();
                                            if !apply_recent.call(entry.clone()) {
                                                return;
                                            }
                                            document::eval("document.querySelector('.create-session-form[role=\"dialog\"]')?.requestSubmit()");
                                        }
                                    },
                                    span { class: "launch-composer-recent-harness", "{entry.selection.harness:?}" }
                                    " · "
                                    span { class: "launch-composer-recent-destination", dir: "ltr", "{display_peer(&entry.cwd)} · {selected_host_label}" }
                                    " · "
                                    // The permission is spelled here, not by the
                                    // summary helper, so the one danger-colored
                                    // word cannot drift from the text around it
                                    // the way string surgery on a Debug spelling
                                    // would. Matched on the signal's value.
                                    span { class: "launch-composer-recent-selection",
                                        "{display_peer(&before_permissions)} · permissions: "
                                        if entry.selection.permissions == Some(LaunchPermission::Yolo) {
                                            span { class: "launch-composer-danger", "yolo" }
                                        } else {
                                            "default"
                                        }
                                    }
                                    span { class: "launch-composer-recent-hint", "⏎ launch" }
                                }
                            }
                            }
                    }
                }
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
            if *creation_surface.read() == CreationSurface::Structured {
                // A destination is a host and its folder, so the structured
                // composer keeps the controls that change either fact in one
                // compact block, in reading order: host and browse, the host
                // reconciliation notes, the folder, the recent-folder links
                // with the two resets, and the optional name. This makes a
                // launch's location reviewable without restoring a second
                // summary of its choices.
                div { class: "launch-composer-destination",
                    span { class: "launch-composer-section-label", "destination" }
                    div { class: "launch-composer-destination-row",
                        {host_select.clone()}
                        button {
                            r#type: "button",
                            disabled: busy || selected.is_none(),
                            // The accessible name stays "browse this path"
                            // while the visible text names the host, so the
                            // control reads as one thing to every client.
                            aria_label: "browse this path",
                            onclick: move |_| {
                                if !draft_transition_allowed(ops) { return; }
                                request_directory_browse(
                                    browse_base_for_folder.clone(), selected, &browse_hosts_for_folder, browse_target,
                                    submitted_field(&cwd(), cwd_edited(), cwd_raw_seed.peek().as_deref()),
                                    browse_generation, browse_request, browse_result, browse_error, browse_reply_completions,
                                    live_browse_connection, cwd, cwd_raw_seed, cwd_edited,
                                );
                            },
                            "browse folders on "
                            span { class: "peer-value", dir: "ltr", "{selected_host_label}" }
                        }
                    }
                    {host_notes.clone()}
                    input {
                        r#type: "text",
                        required: true,
                        autocomplete: "off",
                        autocorrect: "off",
                        autocapitalize: "none",
                        spellcheck: "false",
                        dir: "ltr",
                        value: "{cwd}",
                        disabled: busy,
                        aria_label: "folder",
                        oninput: move |evt| {
                            if !draft_transition_allowed(ops) { return; }
                            promote_fetched_history_snapshot(
                                offered_history, create_target, fetched_history,
                            );
                            cwd.set(evt.value());
                            cwd_edited.set(true);
                            remembered_destination.set(None);
                            invalidate_directory_browse(
                                browse_generation, browse_request, browse_result, browse_error,
                            );
                            intent_key.set(None);
                        },
                    }
                    // Recent folders are destination shortcuts, rendered as
                    // text links rather than chips so they read as history
                    // beneath the field they fill, not as a second picker.
                    div { class: "launch-composer-folder-links", aria_label: "recent folders",
                        if !recent_history.folders.is_empty() {
                            span { class: "launch-composer-folder-links-muted", "recent:" }
                        }
                        for folder in recent_history.folders.iter().take(3) {
                            button {
                                r#type: "button",
                                class: if submitted_field(&cwd(), cwd_edited(), cwd_raw_seed.peek().as_deref()) == folder.display_cwd { "selected" } else { "" },
                                aria_pressed: submitted_field(&cwd(), cwd_edited(), cwd_raw_seed.peek().as_deref()) == folder.display_cwd,
                                disabled: busy,
                                onclick: {
                                    let folder = folder.display_cwd.clone();
                                    let history_target = current_history_target.clone();
                                    move |_| {
                                        if !draft_transition_allowed(ops) { return; }
                                        if !admit_history_destination(
                                            history_target.clone(), live_destination,
                                            remembered_destination, history_activation_attempts,
                                        ) { return; }
                                        promote_fetched_history_snapshot(
                                            offered_history, create_target, fetched_history,
                                        );
                                        invalidate_directory_browse(
                                            browse_generation, browse_request, browse_result, browse_error,
                                        );
                                        reseed_cloned_field(&mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &folder);
                                        intent_key.set(None);
                                    }
                                },
                                "{display_peer(&folder.display_cwd)}"
                            }
                        }
                        if !recent_history.folders.is_empty() {
                            span { class: "launch-composer-folder-links-separator", aria_hidden: "true", "·" }
                        }
                        {destination_resets.clone()}
                    }
                    // This is one of two conditional render sites for the
                    // same optional name signal. Keeping only the active
                    // surface's input mounted gives assistive technology one
                    // destination block and preserves the strict label lookup
                    // tests rely on.
                    label { class: "launch-composer-name",
                        "name (optional)"
                        input {
                            r#type: "text",
                            autocomplete: "off",
                            autocorrect: "off",
                            autocapitalize: "none",
                            spellcheck: "false",
                            // A clone can seed this from a peer-supplied
                            // title, using the same escaped-display/raw-seed
                            // model as the folder field above.
                            dir: "ltr",
                            value: "{title}",
                            disabled: busy,
                            oninput: move |evt| {
                                if !draft_transition_allowed(ops) {
                                    return;
                                }
                                title.set(evt.value());
                                title_edited.set(true);
                                // An edit makes the next submit a DIFFERENT
                                // intent, so the key the last one used stops
                                // applying here (this component's docs carry
                                // the full argument for both edges of that
                                // rule).
                                intent_key.set(None);
                            },
                        }
                    }
                }
                div { class: "launch-composer-choice launch-composer-harness-choice",
                    span { "harness" }
                    div { class: "launch-composer-options",
                        for (harness, label) in [
                            (LaunchHarness::Codex, "Codex"),
                            (LaunchHarness::Claude, "Claude"),
                            (LaunchHarness::Muse, "Muse"),
                            (LaunchHarness::OpenCode, "OpenCode"),
                        ] {
                            button {
                                r#type: "button",
                                class: if *structured_harness.read() == Some(harness) { "selected" } else { "" },
                                aria_pressed: *structured_harness.read() == Some(harness),
                                disabled: busy,
                                onclick: {
                                    let catalog = catalog_for_harness.clone();
                                    move |_| {
                                    if !draft_transition_allowed(ops) {
                                        return;
                                    }
                                    promote_fetched_history_snapshot(
                                        offered_history, create_target, fetched_history,
                                    );
                                    let selection = LaunchSelection {
                                        harness: structured_harness().unwrap_or(harness),
                                        model: structured_model(),
                                        effort: structured_effort(),
                                        permissions: structured_permissions(),
                                    };
                                    let (selection, owner) = crate::launch_composer::reconcile_harness_selection(
                                        selection, *custom_model_harness.peek(), harness, &catalog,
                                    );
                                    composer_reset_reason.set(
                                        crate::launch_composer::reconciliation_reset_reason(
                                            &LaunchSelection {
                                                harness: structured_harness().unwrap_or(harness),
                                                model: structured_model(),
                                                effort: structured_effort(),
                                                permissions: structured_permissions(),
                                            },
                                            &selection,
                                        ),
                                    );
                                    structured_harness.set(Some(selection.harness));
                                    structured_model_raw_seed.set(selection.model.clone());
                                    structured_model_edited.set(false);
                                    structured_model.set(selection.model);
                                    structured_effort.set(selection.effort);
                                    custom_model_harness.set(owner);
                                    intent_key.set(None);
                                    }
                                },
                                // Keep the checkmark's text advance in every
                                // button. CSS only chooses whether it is
                                // visible, so a selected harness cannot move
                                // its unchanged peers.
                                span {
                                    class: "launch-composer-option-check",
                                    aria_hidden: "true",
                                    "✓"
                                }
                                "{label}"
                            }
                        }
                        button {
                            r#type: "button",
                            disabled: busy,
                            onclick: move |_| {
                                if !draft_transition_allowed(ops) {
                                    return;
                                }
                                promote_fetched_history_snapshot(
                                    offered_history, create_target, fetched_history,
                                );
                                creation_surface.set(CreationSurface::Legacy);
                                // This is an explicit escape from the
                                // structured composer, not merely a change
                                // in which controls are visible. Leaving a
                                // remembered profile selected would make a
                                // later Launch run that profile instead of
                                // the command path the button promised.
                                chosen_profile.set(Some(AgentChoice::Command));
                                clone_agent_state.set(CloneAgentState::UserTookOver);
                                intent_key.set(None);
                            },
                            "other / command"
                        }
                    }
                }
                div { class: "launch-composer-choice launch-composer-model-choice",
                    span { "model" }
                    div { class: "launch-composer-options",
                        if structured_harness() != Some(LaunchHarness::OpenCode) {
                        button {
                            r#type: "button",
                            class: if structured_model.read().is_none() { "selected" } else { "" },
                            aria_pressed: structured_model.read().is_none(),
                            disabled: busy,
                            onclick: move |_| {
                                if !draft_transition_allowed(ops) {
                                    return;
                                }
                                promote_fetched_history_snapshot(
                                    offered_history, create_target, fetched_history,
                                );
                                structured_model.set(None);
                                structured_model_raw_seed.set(None);
                                structured_model_edited.set(false);
                                custom_model_harness.set(None);
                                intent_key.set(None);
                            },
                            span {
                                class: "launch-composer-option-check",
                                aria_hidden: "true",
                                "✓"
                            }
                            "harness default"
                        }
                        }
                        // Before choosing a harness, ownership remains in
                        // the label. Once chosen, narrow this stable catalog
                        // order to compatible models instead of leaving
                        // unrelated buttons that would replace the choice.
                        for model in catalog_models.iter().filter(|model| {
                            structured_harness().is_none_or(|harness| harness == model.harness)
                        }) {
                            button {
                                key: "{model.id}",
                                r#type: "button",
                                class: if structured_model.read().as_deref() == Some(model.id.as_str()) { "selected" } else { "" },
                                aria_pressed: structured_model.read().as_deref() == Some(model.id.as_str()),
                                disabled: busy,
                                onclick: {
                                    let model = model.clone();
                                    move |_| {
                                        if !draft_transition_allowed(ops) {
                                            return;
                                        }
                                        promote_fetched_history_snapshot(
                                            offered_history, create_target, fetched_history,
                                        );
                                        structured_harness.set(Some(model.harness));
                                        structured_model_raw_seed.set(None);
                                        structured_model_edited.set(true);
                                        structured_model.set(Some(model.id.clone()));
                                        custom_model_harness.set(None);
                                        let chosen_effort = *structured_effort.peek();
                                        if let Some(effort) = chosen_effort
                                            && !model.efforts.contains(&effort)
                                        {
                                            structured_effort.set(None);
                                            composer_reset_reason.set(Some(
                                                "the selected effort is not in Farhelm's offering for that model, so it was cleared".to_string(),
                                            ));
                                        } else {
                                            composer_reset_reason.set(None);
                                        }
                                        intent_key.set(None);
                                    }
                                },
                                span {
                                    class: "launch-composer-option-check",
                                    aria_hidden: "true",
                                    "✓"
                                }
                                "{display_peer(&model.id)}"
                                if structured_harness().is_none() {
                                    " ({model.harness:?})"
                                }
                            }
                        }
                    }
                    details { class: "launch-composer-more",
                        summary { "more / custom model id" }
                        input {
                        r#type: "text",
                        placeholder: "custom model id",
                        autocomplete: "off",
                        autocorrect: "off",
                        autocapitalize: "none",
                        spellcheck: "false",
                        value: if structured_model_edited() {
                            structured_model.read().as_deref().unwrap_or("").to_string()
                        } else {
                            structured_model_raw_seed().as_deref()
                                .map(display_peer)
                                .unwrap_or_else(|| structured_model.read().as_deref().unwrap_or("").to_string())
                        },
                        disabled: busy || structured_harness.read().is_none(),
                        oninput: move |evt| {
                            if !draft_transition_allowed(ops) {
                                return;
                            }
                            promote_fetched_history_snapshot(
                                offered_history, create_target, fetched_history,
                            );
                            let model = (!evt.value().trim().is_empty()).then(|| evt.value());
                            structured_model_raw_seed.set(None);
                            structured_model_edited.set(true);
                            structured_model.set(model.clone());
                                            custom_model_harness.set(model.as_ref().and_then(|_| *structured_harness.peek()));
                            if let Some(harness) = *structured_harness.peek() {
                                let selection = LaunchSelection {
                                    harness,
                                    model,
                                    effort: *structured_effort.peek(),
                                    permissions: *structured_permissions.peek(),
                                };
                                let before = selection.clone();
                                if !crate::launch_composer::selection_is_compatible(
                                    &selection,
                                    &catalog_for_custom_model,
                                ) {
                                    structured_effort.set(None);
                                    composer_reset_reason.set(
                                        crate::launch_composer::reconciliation_reset_reason(
                                            &before,
                                            &LaunchSelection { effort: None, ..selection },
                                        ),
                                    );
                                } else {
                                    composer_reset_reason.set(None);
                                }
                            }
                            intent_key.set(None);
                        },
                        }
                    }
                }
                if structured_harness() != Some(LaunchHarness::OpenCode) {
                div { class: "launch-composer-choice launch-composer-effort-choice",
                    span { "effort" }
                    div { class: "launch-composer-options",
                        button {
                            r#type: "button", class: if structured_effort.read().is_none() { "selected" } else { "" },
                            aria_pressed: structured_effort.read().is_none(), disabled: busy,
                            onclick: move |_| {
                                if !draft_transition_allowed(ops) { return; }
                                promote_fetched_history_snapshot(
                                    offered_history, create_target, fetched_history,
                                );
                                structured_effort.set(None); intent_key.set(None);
                            },
                            span {
                                class: "launch-composer-option-check",
                                aria_hidden: "true",
                                "✓"
                            }
                            "harness default"
                        }
                        for effort in structured_efforts {
                            button {
                                key: "{effort_value(effort)}", r#type: "button",
                                class: if *structured_effort.read() == Some(effort) { "selected" } else { "" },
                                aria_pressed: *structured_effort.read() == Some(effort), disabled: busy,
                                onclick: move |_| {
                                    if !draft_transition_allowed(ops) { return; }
                                    promote_fetched_history_snapshot(
                                        offered_history, create_target, fetched_history,
                                    );
                                    structured_effort.set(Some(effort)); intent_key.set(None);
                                },
                                span {
                                    class: "launch-composer-option-check",
                                    aria_hidden: "true",
                                    "✓"
                                }
                                "{effort_value(effort)}"
                            }
                        }
                    }
                }
                }
                div { class: "launch-composer-choice launch-composer-permissions-choice",
                    span { "permissions" }
                    div { class: "launch-composer-options",
                        button {
                            r#type: "button", class: if structured_permissions.read().is_none() { "selected" } else { "" },
                            aria_pressed: structured_permissions.read().is_none(), disabled: busy,
                            onclick: move |_| {
                                if !draft_transition_allowed(ops) { return; }
                                promote_fetched_history_snapshot(
                                    offered_history, create_target, fetched_history,
                                );
                                structured_permissions.set(None); intent_key.set(None);
                            },
                            span {
                                class: "launch-composer-option-check",
                                aria_hidden: "true",
                                "✓"
                            }
                            "harness default"
                        }
                        button {
                            r#type: "button", class: if *structured_permissions.read() == Some(LaunchPermission::Yolo) { "selected" } else { "" },
                            aria_pressed: *structured_permissions.read() == Some(LaunchPermission::Yolo), disabled: busy,
                            onclick: move |_| {
                                if !draft_transition_allowed(ops) { return; }
                                promote_fetched_history_snapshot(
                                    offered_history, create_target, fetched_history,
                                );
                                structured_permissions.set(Some(LaunchPermission::Yolo)); intent_key.set(None);
                            },
                            span {
                                class: "launch-composer-option-check",
                                aria_hidden: "true",
                                "✓"
                            }
                            "YOLO"
                        }
                    }
                }
                if let Some(reason) = structured_choice_error {
                    div { class: "launch-composer-choice-error", "{reason}" }
                }
                if let Some(reason) = composer_reset_reason() {
                    div { class: "launch-composer-choice-error", role: "status", "{reason}" }
                }
            } else {
                // Legacy profiles and arbitrary commands remain available for
                // compatibility, but they are an explicit alternate surface
                // rather than a disabled row mixed into the ordinary launch
                // choices. Returning here preserves the draft fields users
                // already entered while making the selected mode visible.
                div { class: "launch-composer-legacy-action",
                    button {
                        r#type: "button",
                        disabled: busy,
                        onclick: move |_| {
                            if !draft_transition_allowed(ops) {
                                return;
                            }
                            creation_surface.set(CreationSurface::Structured);
                            intent_key.set(None);
                            error.set(None);
                        },
                        "back to harnesses"
                    }
                }
                // The legacy surface keeps a plain labelled host control ahead
                // of its working-directory field below; only the structured
                // surface presents host and folder as the destination block.
                // Same Element, same notes, so the two surfaces cannot drift.
                label { class: "launch-composer-host",
                    "host"
                    {host_select.clone()}
                }
                {host_notes.clone()}
            }
            // The agent, offered from the helm catalog and defaulting
            // to what a session was last created from on this helm (SPEC.md's
            // creation rule; `profiles::resolve_agent`). The empty option is
            // the raw command path below rather than "no agent" — a create
            // always launches something, and this select is which of the two
            // mutually exclusive modes it uses.
            label { class: if *creation_surface.read() == CreationSurface::Structured { "launch-composer-legacy-hidden" } else { "" },
                "agent"
                select {
                    class: "create-session-agent",
                    class: "create-session-profile",
                    // Inert for the whole round trip, exactly like the host
                    // selector and for the same reason: the idempotency key
                    // is bound to what is launched, so a selection that moved
                    // between minting and sending would publish a key
                    // belonging to a different create.
                    disabled: busy || *creation_surface.read() == CreationSurface::Structured,
                    value: "{chosen_agent}",
                    onchange: move |evt| {
                        if !draft_transition_allowed(ops) {
                            return;
                        }
                        chosen_profile.set(AgentChoice::from_value(&evt.value()));
                        clone_agent_state.set(CloneAgentState::UserTookOver);
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
                            if profile.builtin {
                                " (Built-in)"
                            }
                        }
                    }
                }
            }
            // SPEC.md's ask-don't-guess fallback, said out loud. It appears
            // only when a profile that WAS available is not anymore — a first
            // create in a helm has nothing to explain — and the thing it
            // rules out is the silent substitution: another profile quietly
            // preselected under the label of a remembered preference.
            if let Some(note) = agent.note {
                div { class: "create-session-profile-note", "{note.text()}" }
            }
            // A catalog that could not be READ is a third state, and it must
            // not look like an empty catalog. Offering only the command path
            // with nothing said would leave a user wondering where their
            // profiles went, so the helm's failure is printed as written.
            if let CatalogLookup::Failed(error) = &held {
                PeerLine {
                    class: "create-session-profile-error".to_string(),
                    parts: vec![
                        DetailPart::text("this helm's profiles could not be read: "),
                        DetailPart::peer(*error),
                    ],
                }
            }
            if let CatalogLookup::Known { refresh_error: Some(error), .. } = &held {
                PeerLine {
                    class: "create-session-profile-refresh-error".to_string(),
                    parts: vec![
                        DetailPart::text(
                            "showing the last catalog this client read; the refresh failed: ",
                        ),
                        DetailPart::peer(*error),
                    ],
                }
            }
            label { class: if *creation_surface.read() == CreationSurface::Structured { "launch-composer-legacy-hidden" } else { "" },
                "working directory"
                input {
                    r#type: "text",
                    required: true,
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    // A clone can seed this from a peer-supplied path (see
                    // `CreatePrefill`), shown ESCAPED while untouched
                    // (`peer::display_peer`, applied by `reseed_cloned_field`)
                    // so a directional override or an invisible character in
                    // it cannot make the field say something different from
                    // the bytes a submit would actually send — the same
                    // escaped-display / raw-seed / edited-flag model
                    // `profiles::ProfileDraft` uses (item2-review2.md F5).
                    // `dir: "ltr"` is the SEPARATE per-value isolation every
                    // other rendering of relayed text gets (`crate::peer`),
                    // so a directional override also cannot visually reorder
                    // this field against the labels and buttons around it.
                    dir: "ltr",
                    value: "{cwd}",
                    disabled: busy,
                    oninput: move |evt| {
                        if !draft_transition_allowed(ops) {
                            return;
                        }
                        cwd.set(evt.value());
                        remembered_destination.set(None);
                        invalidate_directory_browse(
                            browse_generation, browse_request, browse_result, browse_error,
                        );
                        // The user is now typing their OWN text, not
                        // reviewing a clone's — a submit from here on sends
                        // exactly what this field shows, not the raw seed
                        // (`profiles::submitted_field`).
                        cwd_edited.set(true);
                        // An edit makes the next submit a DIFFERENT
                        // intent, so the key the last one used stops
                        // applying here (this component's docs carry the
                        // full argument for both edges of that rule).
                        intent_key.set(None);
                    },
                }
            }
            button {
                r#type: "button",
                class: if *creation_surface.read() == CreationSurface::Structured { "launch-composer-legacy-hidden" } else { "" },
                disabled: busy || selected.is_none(),
                onclick: move |_| {
                    if !draft_transition_allowed(ops) {
                        return;
                    }
                    request_directory_browse(
                        browse_base.clone(),
                        selected,
                        &browse_hosts,
                        browse_target,
                        submitted_field(&cwd(), cwd_edited(), cwd_raw_seed.peek().as_deref()),
                        browse_generation, browse_request, browse_result, browse_error, browse_reply_completions,
                        live_browse_connection, cwd, cwd_raw_seed, cwd_edited,
                    );
                },
                "browse this path"
            }
            if *creation_surface.read() == CreationSurface::Legacy {
                // The legacy destination is its working-directory field plus
                // the browse button above. The two resets follow them, as
                // they do on the structured surface, and then this mirror
                // mounts the same optional-name behavior, so both surfaces
                // read host, folder, browse, resets, name in the same order.
                // The two name sites are surface-guarded so exactly ONE name
                // input exists in the DOM at a time: a hidden duplicate would
                // double the accessible form and break strict label lookups,
                // which is why this is not a `launch-composer-legacy-hidden`
                // twin like the working-directory and command fields.
                div { class: "launch-composer-folder-links",
                    {destination_resets.clone()}
                }
                label { class: "launch-composer-name",
                    "name (optional)"
                    input {
                        r#type: "text",
                        autocomplete: "off",
                        autocorrect: "off",
                        autocapitalize: "none",
                        spellcheck: "false",
                        // Same escaped-display / raw-seed model and the same
                        // per-value directional isolation as the structured
                        // copy above; see that input's comments.
                        dir: "ltr",
                        value: "{title}",
                        disabled: busy,
                        oninput: move |evt| {
                            if !draft_transition_allowed(ops) {
                                return;
                            }
                            title.set(evt.value());
                            title_edited.set(true);
                            // An edit is a different intent, so the last
                            // submit's idempotency key stops applying.
                            intent_key.set(None);
                        },
                    }
                }
            }
            if let Some(reason) = browse_error.read().clone() {
                PeerLine {
                    class: "create-session-error".to_string(),
                    parts: vec![DetailPart::Peer(reason)],
                }
            }
            if let Some((result_authority, result)) = browse_result.read().clone()
                // A completed reply must continue to pass the same test at
                // render and activation time. This covers a path chosen via
                // search or recents, whose handler can run before an effect
                // has had a chance to clear the old browser state.
                && browse_reply_is_current(
                    &browse_request(),
                    &result_authority,
                    browse_target(),
                    live_browse_connection(),
                    &submitted_field(
                        &browse_cwd(),
                        browse_cwd_edited(),
                        browse_cwd_raw_seed.peek().as_deref(),
                    ),
                )
            {
                div { class: "launch-composer-browser",
                    button {
                        r#type: "button",
                        dir: "ltr",
                        onclick: {
                            let selected_cwd = result.cwd.clone();
                            let authority = result_authority.clone();
                            move |_| {
                                browse_activation_attempts.with_mut(|attempts| *attempts = attempts.wrapping_add(1));
                                if !draft_transition_allowed(ops)
                                    || !browse_activation_is_current(
                                        browse_request, &authority, browse_target,
                                        live_browse_connection, cwd, cwd_raw_seed, cwd_edited,
                                    )
                                {
                                    return;
                                }
                                promote_fetched_history_snapshot(
                                    offered_history, create_target, fetched_history,
                                );
                                reseed_cloned_field(&mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &selected_cwd);
                                remembered_destination.set(None);
                                invalidate_directory_browse(
                                    browse_generation, browse_request, browse_result, browse_error,
                                );
                                intent_key.set(None);
                            }
                        },
                        "use {display_peer(&result.cwd)}"
                    }
                    if let Some(parent) = result.parent.clone() {
                        button {
                            r#type: "button",
                            dir: "ltr",
                            onclick: {
                                let authority = result_authority.clone();
                                move |_| {
                                browse_activation_attempts.with_mut(|attempts| *attempts = attempts.wrapping_add(1));
                                if !draft_transition_allowed(ops)
                                    || !browse_activation_is_current(
                                        browse_request, &authority, browse_target,
                                        live_browse_connection, cwd, cwd_raw_seed, cwd_edited,
                                    )
                                {
                                    return;
                                }
                                promote_fetched_history_snapshot(
                                    offered_history, create_target, fetched_history,
                                );
                                reseed_cloned_field(&mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &parent);
                                remembered_destination.set(None);
                                invalidate_directory_browse(
                                    browse_generation, browse_request, browse_result, browse_error,
                                );
                                intent_key.set(None);
                            }
                            },
                            "parent: {display_peer(&parent)}"
                        }
                    }
                    if result.truncated {
                        div { class: "launch-composer-browser-truncated", "more directories exist; refine the path and browse again" }
                    }
                    for child in result.children {
                        button {
                            r#type: "button",
                            dir: "ltr",
                            onclick: {
                                let authority = result_authority.clone();
                                move |_| {
                                browse_activation_attempts.with_mut(|attempts| *attempts = attempts.wrapping_add(1));
                                if !draft_transition_allowed(ops)
                                    || !browse_activation_is_current(
                                        browse_request, &authority, browse_target,
                                        live_browse_connection, cwd, cwd_raw_seed, cwd_edited,
                                    )
                                {
                                    return;
                                }
                                promote_fetched_history_snapshot(
                                    offered_history, create_target, fetched_history,
                                );
                                reseed_cloned_field(&mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &child);
                                remembered_destination.set(None);
                                invalidate_directory_browse(
                                    browse_generation, browse_request, browse_result, browse_error,
                                );
                                intent_key.set(None);
                            }
                            },
                            "{display_peer(&child)}"
                        }
                    }
                }
            }
            // Present in both modes and INERT in one: a profile already says
            // what to run, the wire refuses a create naming both, and the
            // profile's own invocation is the only honest value to show while
            // the field cannot be edited. `required` follows the mode for the
            // same reason — an empty command is exactly right when a profile
            // supplies it.
            label { class: if *creation_surface.read() == CreationSurface::Structured { "launch-composer-legacy-hidden" } else { "" },
                if by_profile {
                    "agent command (the selected profile's own; choose \"custom command\" above to edit)"
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
                    // In custom mode this shows the clone's escaped raw seed
                    // or the user's edit, while profile mode replaces the
                    // display with the selected definition's escaped
                    // invocation. Submission still reads the raw-seed model
                    // below, never this profile-mode presentation.
                    dir: "ltr",
                    value: "{displayed_invocation}",
                    disabled: busy || by_profile || *creation_surface.read() == CreationSurface::Structured,
                    oninput: move |evt| {
                        if !draft_transition_allowed(ops) {
                            return;
                        }
                        invocation.set(evt.value());
                        invocation_edited.set(true);
                        // Typing a command IS choosing the command path, and
                        // recording it here is what keeps a late arrival from
                        // taking the choice away: without it, a catalog
                        // landing a moment later could seed a remembered
                        // profile, disable this very field, and leave what was
                        // just typed on screen but unused. The user said what
                        // they want by typing it.
                        chosen_profile.set(Some(AgentChoice::Command));
                        clone_agent_state.set(CloneAgentState::UserTookOver);
                        // An edit makes the next submit a DIFFERENT
                        // intent, so the key the last one used stops
                        // applying here (this component's docs carry the
                        // full argument for both edges of that rule).
                        intent_key.set(None);
                    },
                }
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
}

#[cfg(test)]
mod tests {
    use super::super::row::row_specimen;
    use super::super::shared::tests::{open, option};
    use super::*;
    use crate::{Profile, SourceProfile};

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
    #[farhelm_testtrace::test]
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
                agent: LaunchIntent::Structured(LaunchSelection {
                    harness: LaunchHarness::Codex,
                    model: None,
                    effort: None,
                    permissions: None,
                }),
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
    /// Submission refuses when the create target and the selected row
    /// disagree about the INSTALL, not merely about the row id.
    ///
    /// This is the rule-level pin of the one-render-lag regression: after a
    /// retarget or an adoption the hosts snapshot describes the successor
    /// while the derived target still holds the predecessor's fingerprint.
    /// The component-level version (staging the submit
    /// handler mid-lag) is not stageable in this harness — the handler lives
    /// inside the dioxus closure — so the comparison is extracted and pinned
    /// here instead.
    #[farhelm_testtrace::test]
    fn submission_requires_the_target_to_match_the_selected_install() {
        let hosts = vec![option(1, "this machine", true)];
        let current = CreateTarget::new(1, "incarnation-1".to_string());
        assert!(target_matches_selection(Some(1), &hosts, Some(&current)));
        assert!(
            !target_matches_selection(
                Some(1),
                &hosts,
                Some(&CreateTarget::new(
                    1,
                    "incarnation-before-retarget".to_string()
                )),
            ),
            "the same row id under a moved install fingerprint is the lag window, not a match"
        );
        assert!(
            !target_matches_selection(Some(2), &hosts, Some(&current)),
            "a selected row absent from the snapshot cannot vouch for any target"
        );
        assert!(
            target_matches_selection(None, &hosts, None),
            "no selection and no target fall through to the hostless refusal downstream"
        );
        assert!(!target_matches_selection(Some(1), &hosts, None));
        assert!(!target_matches_selection(None, &hosts, Some(&current)));
    }

    /// A never-connected host yields NO connection claim, and a connected
    /// one yields exactly its token.
    ///
    /// `Host::incarnation == 0` is the never-connected sentinel, and sending
    /// it as `expected_incarnation: 0` would turn "I observed nothing" into
    /// a precondition the host's very first connection then fails — a create
    /// racing that first connect would be refused as stale with nothing
    /// actually wrong.
    #[farhelm_testtrace::test]
    fn a_never_connected_host_makes_no_connection_claim() {
        let mut connected = option(1, "connected", true);
        connected.connection = 7;
        let mut fresh = option(2, "never connected", false);
        fresh.connection = 0;
        let hosts = vec![connected, fresh];
        assert_eq!(connection_claim(&hosts, 1), Some(7));
        assert_eq!(
            connection_claim(&hosts, 2),
            None,
            "the sentinel is the absence of a claim, not a claim of zero"
        );
        assert_eq!(connection_claim(&hosts, 3), None);
    }

    /// Suggestions may stay visible only while they belong to the host
    /// installation the dialog still targets.
    ///
    /// A retarget retains a numeric host id, so comparing only that id would
    /// show the predecessor's recent launches after the successor arrives.
    #[farhelm_testtrace::test]
    fn history_target_changes_when_a_selected_host_is_retargeted() {
        let before = vec![option(1, "remote", false)];
        let mut after = before.clone();
        after[0].incarnation = "incarnation-after-retarget".to_string();

        assert_ne!(
            history_target(&before, Some(1)),
            history_target(&after, Some(1)),
            "a late history response for the predecessor must not match the successor"
        );
        assert_eq!(history_target(&after, None), None);
    }

    /// Remembered paths survive a connection replacement within one install,
    /// but never follow its registry row onto a different installation. Losing
    /// the row is also a refusal; only an explicit destination choice removes
    /// the historical claim and lets ordinary host validation take over.
    #[farhelm_testtrace::test]
    fn remembered_destination_survives_only_same_install_reconnections() {
        let mut hosts = vec![option(7, "remote", false)];
        hosts[0].connection = 31;
        let remembered = history_target(&hosts, Some(7)).unwrap();
        hosts[0].connection = 32;
        let reconnected = history_target(&hosts, Some(7)).unwrap();
        assert!(remembered_destination_matches(
            Some(&remembered),
            Some(&reconnected),
        ));

        hosts[0].incarnation = "replacement-install".to_string();
        let replacement = history_target(&hosts, Some(7)).unwrap();
        assert!(!remembered_destination_matches(
            Some(&remembered),
            Some(&replacement),
        ));
        assert!(!remembered_destination_matches(Some(&remembered), None));
        assert!(remembered_destination_matches(None, Some(&replacement)));
    }

    /// Browse authority expires on retarget, reconnect, path change, or a
    /// newer generation, even when a stale result is already rendered.
    ///
    /// These are independent changes in the real UI: a registry row can keep
    /// its id through a reconnect; raw-path inequality is a direct mismatch;
    /// and A→B→A matters because its restored text still has a newer
    /// generation. Keeping them together pins the single predicate all three
    /// runtime phases use rather than testing a weaker completion-only guard.
    #[farhelm_testtrace::test]
    fn browse_reply_requires_the_same_live_destination_and_generation() {
        let before = history_target(&[option(1, "remote", false)], Some(1)).unwrap();
        let mut after_hosts = vec![option(1, "remote", false)];
        after_hosts[0].incarnation = "incarnation-after-retarget".to_string();
        let after = history_target(&after_hosts, Some(1));
        let request = BrowseAuthority {
            target: before.clone(),
            connection: Some(4),
            cwd: "/work".to_string(),
            generation: 9,
        };

        assert!(browse_reply_is_current(
            &Some(request.clone()),
            &request,
            Some(before.clone()),
            Some(4),
            "/work",
        ));
        assert!(
            !browse_reply_is_current(&Some(request.clone()), &request, after, Some(4), "/work"),
            "the same registry id cannot make a predecessor directory listing current"
        );
        assert!(
            !browse_reply_is_current(
                &Some(request.clone()),
                &request,
                Some(before.clone()),
                Some(5),
                "/work",
            ),
            "a same-id reconnect must be checked against the live connection, not the request"
        );
        assert!(
            !browse_reply_is_current(
                &Some(request.clone()),
                &request,
                Some(before.clone()),
                Some(4),
                "/other",
            ),
            "a changed raw path must not re-authorize an old directory result"
        );
        let newer = BrowseAuthority {
            generation: 10,
            ..request.clone()
        };
        assert!(
            !browse_reply_is_current(&Some(newer), &request, Some(before), Some(4), "/work",),
            "a newer request generation must not authorize an older result"
        );
    }

    #[farhelm_testtrace::test]
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
    /// the local row otherwise — one answer used by the dialog and create
    /// request validation.
    ///
    /// The middle case is why this is a function rather than two expressions:
    /// a chosen host leaving the registry moves the create target, and every
    /// field that binds the request must agree about that move.
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
    fn precedence_holds_with_all_three_candidates_present() {
        let hosts = vec![
            option(1, "this machine", true),
            option(2, "user@box", false),
            option(3, "user@other", false),
        ];
        assert_eq!(
            effective_create_host(&hosts, Some(3), Some(&open(2))),
            Some(3),
            "a valid explicit choice beats the open session's host"
        );
        assert_eq!(
            effective_create_host(&hosts, Some(99), Some(&open(2))),
            Some(2),
            "a vanished choice falls back to the open session's host, not to local"
        );
        assert_eq!(
            effective_create_host(&hosts, Some(99), Some(&open(98))),
            Some(1),
            "and only when BOTH are gone does the local row answer"
        );
    }

    /// A profile snapshot builds a source in whatever existence the test
    /// wants to pretend the catalog currently reports.
    fn source(existence: ProfileExistence) -> SourceProfile {
        SourceProfile {
            id: "profile-1".to_string(),
            name: "shipped profile".to_string(),
            existence,
        }
    }

    /// A clone trusts the row's own profile choice ONLY while the catalog
    /// still recognizes it under its snapshotted id AND name — every other
    /// existence answer, and no profile at all, falls back to the raw
    /// invocation the row actually ran.
    ///
    /// This is the one decision `PrefillAgent` exists to encode, and it is
    /// STRICTER than `profiles::resolve_agent`'s own rule for an ordinary
    /// create (see `PrefillAgent`'s own doc for why a `Renamed` id is not
    /// trusted here even though `resolve_agent` would accept it): a clone
    /// can be arbitrarily old, so an id merely still existing is not enough
    /// evidence that recreating it is what the current catalog would still
    /// offer under that name.
    #[farhelm_testtrace::test]
    fn prefill_from_clones_the_profile_only_when_it_is_present() {
        let present = Session {
            source_profile: Some(source(ProfileExistence::Present)),
            ..row_specimen("s1")
        };
        assert_eq!(
            prefill_from(&present, 1).agent,
            PrefillAgent::Profile {
                id: "profile-1".to_string()
            }
        );

        for existence in [
            ProfileExistence::Renamed,
            ProfileExistence::Deleted,
            ProfileExistence::Unrecognized,
        ] {
            let session = Session {
                source_profile: Some(source(existence)),
                ..row_specimen("s1")
            };
            assert_eq!(
                prefill_from(&session, 1).agent,
                PrefillAgent::Command,
                "{existence:?} does not name a definition this clone may trust"
            );
        }

        let no_profile = Session {
            source_profile: None,
            ..row_specimen("s1")
        };
        assert_eq!(
            prefill_from(&no_profile, 1).agent,
            PrefillAgent::Command,
            "a raw-invocation session clones its invocation, not a profile it never had"
        );
    }

    /// The raw invocation is carried on every prefill, REGARDLESS of which
    /// mode `agent` trusts — including a profile-backed clone, which has no
    /// use for it until the user switches the mounted form to "custom
    /// command" (see `CreatePrefill::invocation`'s own doc for why leaving
    /// it unset there would let a stale, unrelated command surface then).
    #[farhelm_testtrace::test]
    fn prefill_from_carries_the_raw_invocation_even_for_a_profile_backed_clone() {
        let session = Session {
            invocation: "claude --resume abc".to_string(),
            source_profile: Some(source(ProfileExistence::Present)),
            ..row_specimen("s1")
        };
        assert_eq!(prefill_from(&session, 1).invocation, "claude --resume abc");
    }

    /// A structured session has durable declarative provenance. Clone uses
    /// that snapshot, including omitted defaults, rather than guessing from
    /// the compiled command that happened to launch the original.
    #[farhelm_testtrace::test]
    fn prefill_from_carries_a_structured_launch_snapshot_verbatim() {
        let launch = LaunchSelection {
            harness: LaunchHarness::Muse,
            model: Some("muse-spark-1.3-contributor".to_string()),
            effort: None,
            permissions: Some(LaunchPermission::Yolo),
        };
        let session = Session {
            invocation: "muse --model muse-spark-1.3-contributor --yolo".to_string(),
            launch: Some(launch.clone()),
            ..row_specimen("structured")
        };

        assert_eq!(prefill_from(&session, 1).launch, Some(launch));
    }

    /// Everything else on a prefill travels off the row unmodified — no
    /// suffix on the title, no rewriting of the directory, the row's own
    /// host and install identity together — and the generation is exactly
    /// what the caller passed in (`ListView` is the one that decides what
    /// counts as a new clone).
    #[farhelm_testtrace::test]
    fn prefill_from_carries_title_cwd_host_and_identity_verbatim() {
        let session = Session {
            cwd: "/work/api".to_string(),
            title: "my session".to_string(),
            host: Some(7),
            host_identity: Some(Some("install-7".to_string())),
            ..row_specimen("s1")
        };
        let prefill = prefill_from(&session, 3);
        assert_eq!(prefill.generation, 3);
        assert_eq!(prefill.host, Some(7));
        assert_eq!(prefill.host_identity, Some(Some("install-7".to_string())));
        assert_eq!(prefill.cwd, "/work/api");
        assert_eq!(prefill.title, "my session");
    }

    // -------------------------------------------------------------
    // `resolve_clone_host`: the pure decision behind a clone's host binding
    // a component or an effect.
    // -------------------------------------------------------------

    /// F1: a clone opened before the host registry has answered must keep
    /// retrying rather than giving up — `prefill_applied` (the text-field
    /// latch in the reseed effect) means the ordinary reseed branch never
    /// revisits this generation, so this function is the only thing left
    /// that can still apply its host once the registry does load.
    #[farhelm_testtrace::test]
    fn resolve_clone_host_keeps_waiting_until_the_registry_loads_then_binds_a_matching_row() {
        let hosts = vec![option(1, "remote", false)];
        let identity = Some(Some("install-1".to_string()));

        // Before the registry has answered: hold, stay `Waiting`.
        let (state, action) = resolve_clone_host(
            CloneHostState::Waiting,
            Some(1),
            &identity,
            false, // hosts_loaded
            &hosts,
            false,
        );
        assert_eq!(state, CloneHostState::Waiting);
        assert_eq!(action, CloneHostAction::Hold);

        // Once it has, and the row's install still matches: bind.
        let (state, action) = resolve_clone_host(
            CloneHostState::Waiting,
            Some(1),
            &identity,
            true,
            &hosts,
            false,
        );
        assert_eq!(state, CloneHostState::Bound);
        assert_eq!(
            action,
            CloneHostAction::Bind(CreateTarget::new(1, "incarnation-1".to_string()))
        );
    }

    /// F1's other half: once the registry HAS answered and the row's
    /// install cannot be confirmed, the clone gives up permanently for this
    /// generation rather than sitting in `Waiting` forever.
    #[farhelm_testtrace::test]
    fn resolve_clone_host_gives_up_once_the_registry_answers_without_a_match() {
        let hosts = vec![option(1, "remote", false)];
        let stale_identity = Some(Some("install-superseded".to_string()));
        let (state, action) = resolve_clone_host(
            CloneHostState::Waiting,
            Some(1),
            &stale_identity,
            true,
            &hosts,
            false,
        );
        assert_eq!(state, CloneHostState::Unconfirmable);
        assert_eq!(action, CloneHostAction::Hold);
    }

    /// A hostless clone never resolves a host, however this function is
    /// reached — the caller starts such a clone straight in
    /// `Unconfirmable` (see the reseed effect's generation-transition
    /// branch), and this pins the same guarantee at the function level too:
    /// `Waiting` with no host to check never produces a `Bind`.
    #[farhelm_testtrace::test]
    fn resolve_clone_host_never_binds_a_hostless_clone() {
        let hosts = vec![option(1, "remote", false)];
        let (state, action) =
            resolve_clone_host(CloneHostState::Waiting, None, &None, true, &hosts, false);
        assert_eq!(state, CloneHostState::Unconfirmable);
        assert_eq!(action, CloneHostAction::Hold);
    }

    /// F3: a `Bound` clone is re-checked on every pass, and withdraws the
    /// instant the row's install stops matching — a retarget or an adopt
    /// landing while the form stays open must not leave the selector
    /// silently naming a machine the clone was never actually taken from.
    #[farhelm_testtrace::test]
    fn resolve_clone_host_withdraws_a_bound_clone_once_its_installation_changes() {
        let identity = Some(Some("install-1".to_string()));
        let retargeted = vec![HostOption {
            identity: Some("install-after-the-retarget".to_string()),
            ..option(1, "remote", false)
        }];
        let (state, action) = resolve_clone_host(
            CloneHostState::Bound,
            Some(1),
            &identity,
            true,
            &retargeted,
            true, // chosen_host is still this generation's own pick
        );
        assert_eq!(state, CloneHostState::Unconfirmable);
        assert_eq!(action, CloneHostAction::Withdraw);
    }

    /// The negative case beside it: while the installation still matches, a
    /// `Bound` clone holds — nothing is touched just because the effect
    /// happened to fire again (a feed notice, an unrelated host's refresh).
    #[farhelm_testtrace::test]
    fn resolve_clone_host_stays_bound_while_its_installation_still_matches() {
        let hosts = vec![option(1, "remote", false)];
        let identity = Some(Some("install-1".to_string()));
        let (state, action) = resolve_clone_host(
            CloneHostState::Bound,
            Some(1),
            &identity,
            true,
            &hosts,
            true,
        );
        assert_eq!(state, CloneHostState::Bound);
        assert_eq!(action, CloneHostAction::Hold);
    }

    /// A `Bound` clone whose selector has moved away from its own pick (the
    /// host `<select>`'s own `onchange` already transitions directly to
    /// `UserTookOver`; this is the same outcome reached from this
    /// function's own side, covering any other path that might move
    /// `chosen_host`) yields without forcing anything — no `Withdraw`,
    /// since there is nothing left of the clone's OWN pick in play to undo.
    #[farhelm_testtrace::test]
    fn resolve_clone_host_yields_once_the_selector_moves_away_from_its_own_pick() {
        let hosts = vec![option(1, "remote", false)];
        let identity = Some(Some("install-1".to_string()));
        let (state, action) = resolve_clone_host(
            CloneHostState::Bound,
            Some(1),
            &identity,
            true,
            &hosts,
            false, // chosen_host no longer names this generation's host
        );
        assert_eq!(state, CloneHostState::UserTookOver);
        assert_eq!(action, CloneHostAction::Hold);
    }

    /// Terminal states stay terminal: once a generation's binding has
    /// failed or been taken over, nothing about a LATER pass — even one
    /// where the row's install would once again match — reopens it. Only a
    /// fresh clone (a new generation, a fresh `Waiting`) tries again.
    #[farhelm_testtrace::test]
    fn resolve_clone_host_never_reopens_a_terminal_state() {
        let hosts = vec![option(1, "remote", false)];
        let identity = Some(Some("install-1".to_string()));
        for terminal in [CloneHostState::Unconfirmable, CloneHostState::UserTookOver] {
            let (state, action) =
                resolve_clone_host(terminal, Some(1), &identity, true, &hosts, true);
            assert_eq!(state, terminal);
            assert_eq!(action, CloneHostAction::Hold);
        }
    }

    /// A live helm catalog confirms a clone's profile even while the host
    /// registry is still delayed. This matters because a later host bind must
    /// not own or overwrite the agent choice anymore.
    #[farhelm_testtrace::test]
    fn clone_agent_seeds_while_host_reconciliation_is_waiting() {
        let hosts = vec![option(1, "remote", false)];
        let identity = Some(Some("install-1".to_string()));
        let (host_state, host_action) = resolve_clone_host(
            CloneHostState::Waiting,
            Some(1),
            &identity,
            false,
            &hosts,
            false,
        );
        assert_eq!(host_state, CloneHostState::Waiting);
        assert_eq!(host_action, CloneHostAction::Hold);

        let catalog = ProfileCatalog {
            profiles: vec![Profile {
                id: "p-1".to_string(),
                builtin: false,
                name: "profile".to_string(),
                invocation: "agent".to_string(),
                agent_kind: "generic".to_string(),
                resume_template: None,
            }],
            default_profile: None,
        };
        let (agent_state, action) = resolve_clone_agent(
            CloneAgentState::Waiting,
            &PrefillAgent::Profile {
                id: "p-1".to_string(),
            },
            Some(&catalog),
        );
        assert_eq!(agent_state, CloneAgentState::Seeded);
        assert_eq!(action, Some(AgentChoice::Profile("p-1".to_string())));
    }

    /// An unconfirmable source host changes only the host result. A profile
    /// confirmed by the helm catalog still seeds because it is valid across
    /// every host that helm manages.
    #[farhelm_testtrace::test]
    fn clone_agent_seeds_when_the_source_host_is_unconfirmable() {
        let hosts = vec![option(1, "remote", false)];
        let stale_identity = Some(Some("superseded-install".to_string()));
        let (host_state, host_action) = resolve_clone_host(
            CloneHostState::Waiting,
            Some(1),
            &stale_identity,
            true,
            &hosts,
            false,
        );
        assert_eq!(host_state, CloneHostState::Unconfirmable);
        assert_eq!(host_action, CloneHostAction::Hold);

        let catalog = ProfileCatalog {
            profiles: vec![Profile {
                id: "p-1".to_string(),
                builtin: false,
                name: "profile".to_string(),
                invocation: "agent".to_string(),
                agent_kind: "generic".to_string(),
                resume_template: None,
            }],
            default_profile: None,
        };
        let (_, action) = resolve_clone_agent(
            CloneAgentState::Waiting,
            &PrefillAgent::Profile {
                id: "p-1".to_string(),
            },
            Some(&catalog),
        );
        assert_eq!(action, Some(AgentChoice::Profile("p-1".to_string())));
    }

    /// Once a person overrides the seeded agent, later host reconciliation
    /// cannot produce any agent action. This pins the post-bind race that used
    /// to replace an explicit command when a delayed host answer landed.
    #[farhelm_testtrace::test]
    fn explicit_agent_override_survives_later_host_reconciliation() {
        let catalog = ProfileCatalog {
            profiles: Vec::new(),
            default_profile: None,
        };
        let (state, action) = resolve_clone_agent(
            CloneAgentState::UserTookOver,
            &PrefillAgent::Command,
            Some(&catalog),
        );
        assert_eq!(state, CloneAgentState::UserTookOver);
        assert_eq!(action, None);

        let hosts = vec![option(1, "remote", false)];
        let (host_state, host_action) = resolve_clone_host(
            CloneHostState::Waiting,
            Some(1),
            &Some(Some("install-1".to_string())),
            true,
            &hosts,
            false,
        );
        assert_eq!(host_state, CloneHostState::Bound);
        assert!(matches!(host_action, CloneHostAction::Bind(_)));
    }

    /// item2-review2.md F5's untouched-vs-edited submission rule, exercised
    /// through THIS file's own reuse of it (a clone's directory, invocation
    /// and title) rather than only through the profile editor's copy — the
    /// two must agree because they share one function
    /// (`profiles::submitted_field`), but only this test pins that this
    /// file's own field handling is wired to it correctly, with a value a
    /// clone could plausibly carry: a right-to-left override inside an
    /// otherwise ordinary invocation.
    #[farhelm_testtrace::test]
    fn a_cloned_fields_untouched_submission_sends_the_original_bytes_not_the_escaped_display() {
        let raw = "claude --resume \u{202E}reversed-arg";
        let display = display_peer(raw);
        assert_ne!(
            display, raw,
            "the escaped display must differ from the raw bytes for this test to mean anything"
        );

        // What `reseed_cloned_field` seeds the field with, and what an
        // UNTOUCHED submit sends back — the original bytes, not the
        // escaped spelling the input box is showing.
        assert_eq!(
            submitted_field(&display, false, Some(raw)),
            raw,
            "untouched: the clone's own bytes travel, exactly as the row ran them"
        );

        // The user retypes EXACTLY the escaped spelling (cleaning up a
        // hostile command is precisely this) — equality against the seed
        // cannot tell that apart from "never touched", so only the edited
        // flag can.
        assert_eq!(
            submitted_field(&display, true, Some(raw)),
            display,
            "edited: the user's own literal text travels, even when it happens to equal the \
             escaped rendering of what was there"
        );
    }
}

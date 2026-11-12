//! One launcher for creating, replacing and restarting a session.
//!
//! Intent binding and host reconciliation live beside the form because they
//! define when an idempotency key still describes the submitted create.
//! Accepted tab choices, one-time inputs and placement live in `draft`; this
//! module keeps preview observations, reactive subscriptions and the ordered
//! host/submission guards. The draft assembles every complete request from the
//! caller's accepted facts, including the variant and checkout frozen at a press.
//! Every semantic edit enters a concrete draft operation. The owner alone
//! publishes changes and settles edit-induced retry retirement; this module
//! keeps submission lifetime, observations and ordered renderer completion.
//! Restart holds source identity and placement, while the draft and shared
//! controls still own its edits, templates, validation and YOLO question. Its
//! local confirmation identity never travels as a create retry key; a narrow
//! bridge leaves terminal reattachment with the persistent session owner.
//! Submitted waits hide this mounted form and release its modal boundary;
//! replies retain the same draft and restore that boundary when input is needed.

mod draft;
use draft::{ModelTargets, PermissionUndo, RecoveryTargets, SearchInputs, SearchTargets};

#[cfg(test)]
use draft::ChoiceOrigin;
use draft::{
    CheckoutInput, ChoiceTargets, DraftFields, FolderTargets, LaunchDraft, LaunchDraftState,
    PlacementTargets, RepositoryTargets, RequestContext, SourceTargets, use_launch_draft,
};

use dioxus::prelude::*;
use farhelm_proto::github_checkout::CheckoutNameSequence;

use crate::api::{self, CreateAgent, create_session, mint_intent_key};
use crate::feed::{fallback_polls_now, fallback_sleep, use_feed_reader};
use crate::github_checkout::{
    DestinationDraft, GithubAttempt, GithubCheckoutRequest, GithubRepo, PreviewAuthority,
    PreviewState, RepositoryAuthority, repository_choices,
};
use crate::launch_activity::{LaunchActivity, launch_is_current};
use crate::launch_controls::{DestinationTypeControl, LaunchControls, enter_choice};
use crate::launch_progress::{LaunchAction, LaunchProgress, PendingLaunch};
use crate::modal_dialog::{
    DialogActions, DialogFocus, DialogFooter, DialogHandle, DialogHeader, DialogRoot, FocusTarget,
    InitialFocus, ModalDialog, RecoveryDelay, RecoveryFocus, ScrollRegion,
};
use crate::ops::{ConfirmSlot, ConfirmToken, OpGuard, OpLock, use_confirm_slot};
use crate::peer::{DetailPart, PeerLine, PeerText, display_peer};
use crate::reader::{SurfaceReader, Trigger, request_read};
use crate::session_ops::{SessionGuard, SessionOps};
use crate::{
    ApiBase, CommandLaunch, HostId, LaunchHarness, LaunchPermission, LaunchSelection, Session,
    SessionStatus,
};

use super::SharedPreferences;
use super::shared::{
    HostOption, OpenHost, effective_create_host, enrich_created_session, matching_host_option,
};

/// One launcher shortcut press, resolved from a single fresh read before handoff.
/// A completed sequence is one refusal unit; its definitions must not be mixed
/// with the next press when a template cannot apply to the current draft.
#[derive(Clone)]
pub(super) struct ShortcutApplyRequest {
    pub(super) opening: u64,
    /// Revokes replay if another modal opened after the physical press.
    pub(super) context: crate::launch_sequence::ShortcutContextSnapshot,
    pub(super) templates: std::collections::VecDeque<farhelm_proto::launcher::LaunchTemplate>,
    pub(super) refusal: Option<String>,
}

/// How many times one submit will mint a key before giving up.
///
/// The retry exists for a queued keystroke landing during the mint, which
/// resolves on the second attempt; anything beyond that is a form whose
/// values keep changing faster than a UUID can be generated, which is not a
/// create anybody is waiting on. Bounded rather than a bare loop because
/// spinning is a worse answer than saying so.
const MINT_ATTEMPTS: usize = 3;

/// The footer's explanation distinguishes a refusal from an unknown outcome.
/// Keeping this in the existing attempted-state slot gives validation and
/// template errors the same reset boundaries they had before timeout wording.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LaunchAttempt {
    None,
    Refused,
    Unknown,
}

impl LaunchAttempt {
    /// Both finished outcomes reveal hidden launchers and use the loud alert.
    /// The unknown variant changes its prose, never its retry identity.
    fn was_attempted(self) -> bool {
        self != Self::None
    }

    /// A deadline cannot claim refusal, even though it uses the same alert.
    /// Ordinary refused attempts retain the mode-specific prefix users know.
    fn prefix(self, restart: bool) -> &'static str {
        match (self, restart) {
            (Self::Refused, true) => "Restart error. ",
            (Self::Refused, false) => "Not launched. ",
            (Self::None | Self::Unknown, _) => "",
        }
    }
}

/// Preparation belongs to one form and one host, through both the vendor
/// request and SSH recovery. It grants no checkout authority: a fresh checkout
/// waits for the form's own preview on the recovered connection before creating.
#[derive(Clone, PartialEq)]
struct HostResumePreparation {
    host: HostId,
    /// Display-escaped and phase-free, captured before suspension changes the row.
    name: String,
}

/// Keep preparation distinct from session creation in both the normal form
/// and the floating quick-launch surface. The captured name has no phase suffix.
#[component]
fn HostResumeProgress(preparation: HostResumePreparation) -> Element {
    rsx! {
        div { class: "host-resume-progress", role: "status",
            span { class: "delete-spinner", aria_hidden: "true" }
            span {
                "Resuming "
                bdi { "{preparation.name}" }
                ", then connecting… "
                "the session starts when it’s ready"
            }
        }
    }
}

/// How long a fresh-checkout launch that woke its host waits for this window to
/// see the woken host connected (SPEC.md "Creation"). The wake itself already
/// confirmed the connection through its own read; this bounds only the page's
/// host list catching up, which could otherwise hold the launch, its hidden
/// launcher and every other launch and host change in the window indefinitely.
const WOKEN_CONNECTION_WAIT_MS: u64 = 60_000;

/// The launcher's error when [`WOKEN_CONNECTION_WAIT_MS`] runs out. Wording
/// agreed with the maintainer; it appears after the "Not launched. " prefix.
const WOKEN_CONNECTION_TIMEOUT: &str = "The host resumed, but this window did not see it connect within 60 seconds. The session was not launched; try again.";

/// Shared by visible invalidation and both submission checks so an explicit
/// destination correction can retire only this refusal, not an unrelated error.
const REMEMBERED_DESTINATION_CHANGED: &str = "The remembered folder belongs to a different installation; choose the host or folder again before launching";

/// An empty field means no prefix; every nonempty draft must parse before a
/// launch or template save so an unfinished quote cannot silently disappear.
fn wrapper_words(text: &str) -> Result<Option<Vec<String>>, String> {
    if text.is_empty() {
        Ok(None)
    } else {
        farhelm_proto::split_wrapper_prefix(text)
            .map(Some)
            .map_err(crate::local_validation_notice)
    }
}

/// Recover this agent's last successful user choice from the shared helm row.
/// Switching agent type discards the prior type's draft rather than carrying
/// its command prefix into a different agent invocation.
/// Join the stored shell spelling: the draft's inherited-value guard owns
/// escaped read-only presentation, separately from the executable bytes.
fn remembered_wrapper(preferences: &api::Preferences, harness: Option<LaunchHarness>) -> String {
    harness
        .and_then(|harness| preferences.remembered_wrapper_prefixes.get(&harness))
        .map(|words| {
            words
                .iter()
                .map(crate::peer::PeerText::raw)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

/// A malformed custom editor needs its own reason, even when catalog
/// compatibility also rejects the resulting incomplete selection.
const CUSTOM_PERMISSION_FLAGS_REFUSAL: &str =
    "Enter valid custom permission flags before launching";

/// The first unmet launch prerequisite, shared by the button and its refusal.
///
/// These are presentation inputs, not admission authority: submit still checks
/// live signals and the helm decides whether the request can actually launch.
/// Keeping one reason for both the grey state and hover prevents a button from
/// advertising readiness while silently refusing an attempt.
struct LaunchPrerequisites {
    busy: bool,
    saving_template: bool,
    host_available: bool,
    destination_ready: bool,
    github_search: bool,
    remembered_destination_valid: bool,
    folder_default_waiting: bool,
    needs_harness: bool,
    invalid_permission_flags: bool,
    incompatible_choice: bool,
    invalid_wrapper: bool,
}

impl LaunchPrerequisites {
    /// Explain the first condition the user must clear before launching.
    /// Ordering keeps transient operation locks ahead of draft corrections.
    fn refusal(&self) -> Option<&'static str> {
        if self.busy {
            Some("Wait for the current operation to finish before launching")
        } else if self.saving_template {
            Some("Finish or cancel saving the template before launching")
        } else if !self.host_available {
            Some("Choose a connected host before launching")
        } else if self.github_search {
            Some("Select a GitHub repository before launching")
        } else if !self.destination_ready {
            Some("Wait for a current checkout preview before launching")
        } else if !self.remembered_destination_valid {
            Some(REMEMBERED_DESTINATION_CHANGED)
        } else if self.folder_default_waiting {
            Some("Wait for this host's last folder before launching")
        } else if self.needs_harness {
            Some("Choose a structured harness before launching")
        } else if self.invalid_permission_flags {
            Some(CUSTOM_PERMISSION_FLAGS_REFUSAL)
        } else if self.incompatible_choice {
            Some(
                "This saved choice is no longer supported by the current catalog; choose a compatible model or effort",
            )
        } else if self.invalid_wrapper {
            Some("Correct the wrapper prefix before launching")
        } else {
            None
        }
    }
}

/// The host installation a create intent is bound to.
///
/// The idempotency key and clone target are installation-specific: a
/// registry row can be retargeted or adopted while retaining its numeric id.
/// This value carries that safety boundary.
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

/// One pending display snapshot, not a claim that this connection is usable.
/// A successful Resume can finish before the form's independent host reader
/// catches up. Its receipt suppresses only that snapshot's obsolete warning;
/// any token, installation, selection, or settled-phase change retires it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ResumeDisplayHandoff {
    target: CreateTarget,
    connection: u64,
}

/// Reduce the currently displayed target to the facts a Resume handoff needs.
/// Settled failures, Suspended, Connected and removed hosts are ineligible, so
/// a receipt cannot conceal their current state even if their token is unchanged.
fn resume_display_observation(
    hosts: &[HostOption],
    selected: Option<HostId>,
) -> Option<ResumeDisplayHandoff> {
    let host = hosts.iter().find(|host| Some(host.id) == selected)?;
    host.connection_pending.then(|| ResumeDisplayHandoff {
        target: CreateTarget::new(host.id, host.incarnation.clone()),
        connection: host.connection,
    })
}

/// A successful helper may bridge only its unchanged target's lagging display.
/// The returned connection proves preparation finished; it never replaces the
/// shared snapshot used for submit admission or checkout-preview authority.
fn completed_resume_display_handoff(
    target: Option<&CreateTarget>,
    displayed: Option<&ResumeDisplayHandoff>,
    fresh: &[HostOption],
) -> Option<ResumeDisplayHandoff> {
    let target = target?;
    let displayed = displayed.filter(|displayed| &displayed.target == target)?;
    let connected = fresh
        .iter()
        .find(|host| host.id == target.host && host.phase.is_none())?;
    (CreateTarget::new(connected.id, connected.incarnation.clone()) == *target)
        .then(|| displayed.clone())
}

/// A recent folder choice carries the installation that offered it. A retained mouse
/// callback must not apply old history to a newly selected or replaced host.
#[derive(Clone)]
enum FolderChoice {
    Recent {
        cwd: String,
        target: Option<CreateTarget>,
    },
    Home,
    LocalHome,
}

/// Keep the server's newest-first ordering, filtering only an edited field.
/// Prefilled clone paths and `~` are seeds, not searches; they show eight recents.
/// Filtering uses original path text, so a visible escape marker never becomes
/// an alias for the control character it represents.
fn folder_suggestions<'a>(
    folders: &'a [api::FolderHistoryEntry],
    text: &str,
    edited: bool,
) -> Vec<&'a api::FolderHistoryEntry> {
    let query = text.to_lowercase();
    folders
        .iter()
        .filter(|folder| !edited || folder.display_cwd.raw().to_lowercase().contains(&query))
        .take(8)
        .collect()
}

/// Keep at most 36 displayed characters without leaving a partial escape marker.
/// Long paths keep up to 35 escaped characters from the end, with a leading
/// ellipsis. A generated marker crossing that boundary is dropped with the prefix,
/// so its orphaned suffix cannot disguise an unsafe character. Literal marker-shaped
/// path text keeps ordinary truncation; the full escaped accessible name and tooltip
/// remain independent of this label.
fn folder_tail_label(path: &PeerText) -> String {
    let visible = path.escaped();
    let count = visible.chars().count();
    if count <= 36 {
        return visible;
    }
    let mut start = count - 35;
    let mut shown = 0;
    // Walk original characters so only generated escapes are indivisible.
    // Parsing the displayed spelling would also change ordinary filenames that
    // happen to contain literal marker text. The shared predicate remains the
    // authority; each unsafe scalar has the same ASCII spelling as display_peer.
    for ch in path.raw().chars() {
        let width = if farhelm_proto::text::is_presentation_unsafe(ch) {
            format!("<U+{:04X}>", ch as u32).len()
        } else {
            1
        };
        let end = shown + width;
        if shown < start && start < end {
            // Keep the final tail and its budget, dropping this whole marker
            // with the omitted prefix. Ordinary one-character runs cannot cross.
            start = end;
            break;
        }
        if end >= start {
            break;
        }
        shown = end;
    }
    format!("…{}", visible.chars().skip(start).collect::<String>())
}

/// Keep an inherited repository's display separate from its request identity.
/// Clone can supply a selected repo without the search parser's validation.
/// Unsafe selections stay read-only until explicitly cleared, so editing the
/// visible escape markers cannot silently replace the original repository.
/// Unselected query text remains the user's ordinary editable draft.
fn repository_field(destination: &DestinationDraft) -> (String, bool) {
    let raw = destination.repository_text();
    let read_only = destination.repo().is_some()
        && raw.chars().any(farhelm_proto::text::is_presentation_unsafe);
    let shown = match destination.repo() {
        Some(repo) if read_only => format!("{}/{}", repo.owner.editable(), repo.name.editable()),
        _ => raw,
    };
    (shown, read_only)
}

/// What a successful create hands to `on_created`: the session the helm
/// returned, the structured launch this form SUBMITTED, if any, and whether
/// New stayed hidden so the caller can offer naming after acceptance.
///
/// Kept apart because they come from different parties. `session.launch` is
/// the supervisor's reply, which a remote host writes; the form's own
/// submission is the user's choice. Anything that remembers "what the user
/// picked" (the permission mirror in `list::view`) must
/// read `submitted_launch` (SPEC.md: only explicit GUI selections shape GUI
/// defaults). `None` for command creates.
pub(crate) struct CreatedSession {
    pub(crate) session: Session,
    pub(crate) submitted_launch: Option<LaunchSelection>,
    /// Naming is eligible only for a hidden quick opening the user still awaits.
    /// A revealed form already offered its name field; intervening page input
    /// or a retired naming admission also suppresses that follow-up dialog.
    pub(crate) new_was_never_shown: bool,
    /// The mounted draft this reply belongs to, independent of later modal epochs.
    pub(crate) opening: u64,
    /// One accepted user answer travels through wake, preview and retries unchanged.
    pub(crate) press_baseline: Option<u64>,
    /// Read at completion, and again after any naming admission wait. A retired
    /// opening/admission supplies no navigation authority even if a count was read.
    pub(crate) press_current: Option<u64>,
}

/// A source host absent from the registry might be a just-created sandbox.
/// Wait for its ordinary metadata instead of borrowing local placement. An
/// explicit different host or recipe is the user's replacement placement.
fn unresolved_creation_origin(
    origin: Option<HostId>,
    hosts: &[HostOption],
    chosen: Option<HostId>,
    recipe: Option<&str>,
) -> bool {
    recipe.is_none()
        && origin.is_some_and(|origin| {
            !hosts.iter().any(|host| host.id == origin)
                && chosen.is_none_or(|chosen| chosen == origin)
        })
}

/// Preserve the opening's actual visibility history at accepted-create time.
/// A refusal, YOLO question or incomplete template reveals New permanently;
/// the initial quick-launch intent alone cannot justify a second naming prompt.
fn new_was_never_shown(quick_origin: bool, quick_launch_hidden: bool) -> bool {
    quick_origin && quick_launch_hidden
}

/// What one create would actually LAUNCH.
///
/// The two creation modes are mutually exclusive on the wire and they are
/// mutually exclusive here for a second reason: they are part of the intent
/// an idempotency key stands for. Keeping the typed command inside the
/// `Command` arm is what makes "a structured create does not care what is in
/// the command box" structural — a form field the user cannot reach in that
/// mode can no longer change what the key is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LaunchIntent {
    /// A command launch as the form's command fields hold it.
    Command(CommandLaunch),
    /// Declarative structured intent compiled only by the helm.
    Structured(LaunchSelection),
}

/// The command launch the form's command fields describe, or `None` while
/// the YOLO assertion is still unanswered: SPEC.md makes it a required
/// choice with no default, so there is nothing to launch until the user
/// makes it. The resume command counts only while Resume is opted into
/// AND an agent type is declared: the Resume controls are shown only with
/// a declared type, so a resume command left behind by clearing the type
/// would be a hidden field the helm refuses and the user cannot see to
/// clear.
fn command_intent(
    command: String,
    yolo: Option<bool>,
    agent: Option<LaunchHarness>,
    resume_on: bool,
    resume: &str,
) -> Option<CommandLaunch> {
    Some(CommandLaunch {
        command,
        yolo: yolo?,
        agent,
        resume: (resume_on && agent.is_some()).then(|| resume.to_string()),
    })
}

/// The launcher's active tab, one per launch kind (SPEC.md: "The two launch
/// kinds are shown as two tabs, agent and command, each showing only its own
/// fields; switching tabs keeps each tab's draft").
///
/// Each tab retains its own draft, so switching never copies a value from one
/// draft into the other: a value from one kind cannot safely become a hidden
/// input to the other's launch or idempotency key, and coming back finds the
/// draft as it was left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LaunchTab {
    /// An agent type and its choices, composed by the helm.
    Agent,
    /// A command line the user writes, with its YOLO answer.
    Command,
}

/// Decode the helm's remembered `remembered_permissions` word into the
/// composer's typed choice, for a dialog mount that has nothing else to
/// seed it from (SPEC.md's launch-composer carve-out: the last SUCCESSFUL
/// structured launch's permissions mode is the one preselected choice a
/// fresh "New" open carries over).
///
/// Deliberately preferences-only, not preferences-and-prefill: a
/// clone/replace prefill's own reseed effect always wins when one is
/// present (decided alongside this feature — a prefill is a more specific,
/// more recent intent than the helm-wide memory), and that precedence is a
/// one-line guard at each of this function's two call sites rather than
/// logic folded in here. There is no decodable behavior in "ignore the
/// memory when a prefill exists" worth unit-testing on its own; what IS
/// worth testing, and what this function isolates, is the word-to-enum
/// decode — including tolerating a word this build does not recognize,
/// mirroring `list::view::decoded_sort`'s tolerance for an unrecognized
/// `list_sort`. The helm's own store refuses words outside the released
/// modes (`is_known_remembered_permissions_word` on the helm side), so this
/// fallback is defense in depth against a stale cached reply or an older UI
/// build talking to a newer helm, not a path this build's own writes can
/// trigger.
fn initial_structured_permissions(preferences: &api::Preferences) -> Option<LaunchPermission> {
    match preferences.remembered_permissions.as_deref() {
        Some("yolo") => Some(LaunchPermission::Yolo),
        Some("guarded") => Some(LaunchPermission::Guarded),
        Some("approve") => Some(LaunchPermission::Approve),
        Some("smart_approve") => Some(LaunchPermission::SmartApprove),
        Some("chat") => Some(LaunchPermission::Chat),
        _ => None,
    }
}

/// Describe reconciliation without promoting a passive remembered mode into
/// an explicit user choice.
fn draft_reconciliation_reason(
    before: &LaunchSelection,
    after: &LaunchSelection,
    permissions_are_explicit: bool,
) -> Option<String> {
    let mut deliberate_before = before.clone();
    if !permissions_are_explicit {
        deliberate_before.permissions = None;
    }
    crate::launch_composer::reconciliation_reset_reason(&deliberate_before, after)
}

/// The host row a template's host field names: the one whose recorded
/// install identity it is, unless that row is in the identity-mismatch
/// phase. There its recorded identity is the PREDECESSOR's while the row
/// reaches a different install, so matching it would aim the template at the
/// successor, which SPEC.md rules out ("a registry row retargeted to another
/// install makes the template's host field inapplicable"). The same
/// exclusion the create default makes (`default_create_host`).
fn template_host<'a>(hosts: &'a [HostOption], identity: &str) -> Option<&'a HostOption> {
    hosts
        .iter()
        .find(|host| !host.identity_mismatch && host.identity.as_deref() == Some(identity))
}

/// One immutable pre-switch draft, kept only if the completed user action
/// escalates permissions. Raw editor seeds and ownership are part of the
/// draft: restoring displayed text alone could change peer-provided bytes.
struct PermissionSwitchBefore {
    harness: Option<LaunchHarness>,
    permission: Option<LaunchPermission>,
    tab: LaunchTab,
    undo: std::rc::Rc<PermissionUndo>,
}

/// At most one banner owns recovery within one launcher opening. The opening
/// generation also fences a queued Undo when a mounted Clone/Replace reseed
/// has superseded its source, before effects have removed the old controls.
#[derive(Clone)]
struct PermissionEscalation {
    generation: u64,
    opening_generation: u64,
    message: String,
    undo: std::rc::Rc<PermissionUndo>,
}

/// Rank the shown mode, including an agent's unclassified own-setting mode.
/// Modes in one group have different mechanisms but no warning-worthy order.
fn permission_danger(permission: Option<LaunchPermission>) -> u8 {
    match permission {
        Some(LaunchPermission::Chat | LaunchPermission::Approve) => 0,
        Some(LaunchPermission::SmartApprove | LaunchPermission::Guarded) => 1,
        Some(LaunchPermission::Yolo | LaunchPermission::Custom) | None => 2,
    }
}

/// Banner wording names the mode itself; detailed vendor behavior remains in
/// the permission control's existing help rather than a second explanation.
fn permission_mode_name(permission: Option<LaunchPermission>) -> &'static str {
    match permission {
        Some(LaunchPermission::Guarded) => "Guarded",
        Some(LaunchPermission::Yolo) => "YOLO",
        Some(LaunchPermission::Custom) => "Custom flags",
        Some(LaunchPermission::Approve) => "Approve",
        Some(LaunchPermission::SmartApprove) => "Smart approve",
        Some(LaunchPermission::Chat) => "Chat",
        None => "Agent's own setting",
    }
}

/// Observe one completed published transaction, preserving conditional signal
/// reads from opening replay. Recovery is immutable data, never a callback.
fn note_completed_permission_switch(
    draft: LaunchDraftState,
    before: PermissionSwitchBefore,
    automatic: bool,
    mut permission_escalation: Signal<Option<PermissionEscalation>>,
    mut permission_escalation_generation: Signal<u64>,
    opening_generation: Signal<u64>,
) {
    let current = draft.snapshot();
    let after = current.harness();
    if before.harness == after {
        return;
    }
    permission_escalation.set(None);
    if !automatic || before.tab != LaunchTab::Agent || current.tab() != LaunchTab::Agent {
        return;
    }
    let Some(harness) = after else {
        return;
    };
    let permission = harness.request_permission(current.permissions());
    if permission_danger(permission) <= permission_danger(before.permission) {
        return;
    }
    permission_escalation_generation += 1;
    permission_escalation.set(Some(PermissionEscalation {
        generation: permission_escalation_generation(),
        opening_generation: *opening_generation.peek(),
        message: format!(
            "Permission changed: {} → {}, because {} does not support {}.",
            permission_mode_name(before.permission),
            permission_mode_name(permission),
            crate::launch_composer::harness_label(harness),
            permission_mode_name(before.permission)
        ),
        undo: before.undo,
    }));
}

/// Project the agent tab's first message into templates and supported requests.
/// Visiting a command or unsupported agent keeps the dormant text in the form,
/// but it cannot silently become an argument to that unrelated launch.
/// An unset agent still retains a partial template's message so a later
/// template can supply the agent without losing the first template's edit.
fn active_prompt(
    kind: farhelm_proto::launcher::LauncherKind,
    harness: Option<LaunchHarness>,
    prompt: &Option<String>,
) -> Option<String> {
    (kind == farhelm_proto::launcher::LauncherKind::Agent
        && harness.is_none_or(|agent| agent.supports_initial_prompt(false)))
    .then(|| prompt.clone())
    .flatten()
}

/// Capture the same raw launcher values for template application and saving.
/// Relayed editor text retains its original bytes until deliberately edited;
/// destination and name are included so saving does not grow a second reader.
/// Unselected repository text is omitted: it has not become a destination yet.
/// A template source contributes its unnumbered base and kind; a resolved
/// suffix belongs to one launch and must not become the next saved base.
fn launcher_snapshot(
    draft: &LaunchDraft,
    hosts: &[HostOption],
    selected: Option<HostId>,
) -> farhelm_proto::launcher::LauncherState {
    use farhelm_proto::launcher::{LauncherKind, LauncherState, TemplateDestination};
    let kind_now = match draft.tab() {
        LaunchTab::Agent => LauncherKind::Agent,
        LaunchTab::Command => LauncherKind::Command,
    };
    LauncherState {
        recipe: None,
        target: draft.target(),
        prompt: active_prompt(kind_now, draft.harness(), &draft.prompt()),
        kind: Some(kind_now),
        harness: draft.harness(),
        model: draft.model().as_ref().map(|model| {
            submitted_field(
                model,
                draft.model_edited(),
                draft.model_raw_seed().as_deref(),
            )
        }),
        model_owner: draft.model_owner(),
        effort: draft.effort(),
        permissions: draft.permissions(),
        permission_flags: crate::launch_composer::permission_words(
            draft.permissions(),
            &draft.permission_flags(),
        ),
        wrapper_prefix: wrapper_words(&draft.wrapper()).ok().flatten(),
        container: draft.container(),
        workspace_trust: None,
        command: submitted_field(
            &draft.invocation(),
            draft.invocation_edited(),
            draft.invocation_raw_seed().as_deref(),
        ),
        yolo: draft.command_yolo(),
        command_agent: draft.command_agent(),
        resume_command: draft.command_resume_on().then(|| {
            submitted_field(
                &draft.command_resume(),
                draft.command_resume_edited(),
                draft.command_resume_raw_seed().as_deref(),
            )
        }),
        host: draft
            .recipe()
            .is_none()
            .then_some(selected)
            .flatten()
            .and_then(|chosen| {
                hosts
                    .iter()
                    .find(|host| host.id == chosen && !host.identity_mismatch)
                    .and_then(|host| host.identity.clone())
            }),
        // An unselected query is an editor draft, not a reusable destination.
        // Saving it would make the whole template fail when applied later.
        destination: match &draft.destination() {
            DestinationDraft::Existing { .. } => {
                Some(TemplateDestination::Folder(submitted_field(
                    &draft.cwd(),
                    draft.cwd_edited(),
                    draft.cwd_raw_seed().as_deref(),
                )))
            }
            DestinationDraft::GithubInput { .. } => None,
            DestinationDraft::Github { repo, .. } => {
                Some(TemplateDestination::Github(repo.identifier()))
            }
        },
        name: Some(
            draft
                .checkout_naming()
                .template_name()
                .map(|(base, _)| base.to_string())
                .unwrap_or_else(|| draft.effective_title()),
        ),
        name_kind: draft
            .checkout_naming()
            .template_name()
            .map(|(_, kind)| kind),
    }
}

/// Remove assignments that repeat values this opening holds fixed.
///
/// These must be absent during both validation and replay: reselecting even
/// the same agent reconciles permissions/model/effort, and reselecting a host
/// resets placement state. Equality is deliberately textual except for folder
/// trailing slashes; the browser cannot resolve remote paths or expand `~`.
/// Search results and recents do not pass through this template-only rule.
fn remove_matching_held_fields(
    fields: &mut farhelm_proto::launcher::TemplateFields,
    held: &farhelm_proto::launcher::LauncherState,
    restart: bool,
    host_fixed: bool,
) {
    use farhelm_proto::launcher::{LauncherKind, TemplateDestination};
    if (restart || host_fixed) && fields.host.is_some() && fields.host == held.host {
        fields.host = None;
    }
    if !restart {
        return;
    }
    if fields.kind.is_some() && fields.kind == held.kind {
        fields.kind = None;
    }
    let agent = match held.kind {
        Some(LauncherKind::Command) => held.command_agent,
        _ => held.harness,
    };
    if fields.agent.is_some() && fields.agent == agent {
        fields.agent = None;
    }
    if fields
        .container
        .as_ref()
        .is_some_and(|config| *config == held.container)
    {
        fields.container = None;
    }
    if let (Some(TemplateDestination::Folder(folder)), Some(TemplateDestination::Folder(current))) =
        (&fields.destination, &held.destination)
        && !folder.is_empty()
        && folder.trim_end_matches('/') == current.trim_end_matches('/')
    {
        fields.destination = None;
    }
    if fields.name.is_some() && fields.name == held.name {
        fields.name = None;
        // A name kind without its name is invalid, and neither part of a
        // repeated name may turn the session's raw title into a template base.
        fields.name_kind = None;
    }
}

/// Keep the projected state, parsed destinations and cleaned replay inputs
/// together. Their vectors retain the input order, including empty entries.
type ValidatedTemplateBatch = (
    farhelm_proto::launcher::LauncherState,
    Vec<Option<crate::github_checkout::GithubRepo>>,
    Vec<farhelm_proto::launcher::LaunchTemplate>,
);

/// Validate a shortcut batch and return the exact templates safe to replay.
/// The shared fold checks ordered applicability, while the launcher also has
/// to parse GitHub destinations before replay can safely choose one. Ordinary
/// `tl:` acceptance uses the same fold. Equal held assignments are removed
/// against the opening snapshot; remaining held-field checks run in order, so
/// a later held change cannot hide an earlier ordinary refusal. No edits land
/// unless the whole batch succeeds.
#[cfg(test)]
fn validated_template_batch(
    templates: &[farhelm_proto::launcher::LaunchTemplate],
    hosts: &[HostOption],
    catalog: &[crate::api::LaunchCatalogModel],
    host_fixed: bool,
    draft: &LaunchDraft,
    chosen_host: Option<HostId>,
) -> Result<ValidatedTemplateBatch, String> {
    validated_template_batch_known(
        templates,
        hosts,
        catalog,
        host_fixed,
        draft,
        chosen_host,
        &[],
    )
}

/// Recipe applicability uses the current catalog while retaining the shared
/// ordered, atomic template fold used for ordinary installed-host targets.
fn validated_template_batch_known(
    templates: &[farhelm_proto::launcher::LaunchTemplate],
    hosts: &[HostOption],
    catalog: &[crate::api::LaunchCatalogModel],
    host_fixed: bool,
    draft: &LaunchDraft,
    chosen_host: Option<HostId>,
    recipes: &[String],
) -> Result<ValidatedTemplateBatch, String> {
    use farhelm_proto::launcher::TemplateDestination;
    let mut state = launcher_snapshot(draft, hosts, chosen_host);
    // A partial template may fill an agent later in this same batch. Preserve
    // its dormant prompt while checking the projected sequence.
    state.prompt = draft.prompt();
    let known_hosts: Vec<String> = hosts
        .iter()
        .filter(|host| !host.identity_mismatch && !host.session_owned)
        .filter_map(|host| host.identity.clone())
        .collect();
    let context = farhelm_proto::launcher::TemplateContext {
        catalog,
        known_hosts: &known_hosts,
        known_recipes: recipes,
        known_places: &hosts
            .iter()
            .filter(|host| !host.session_owned)
            .map(|host| farhelm_proto::Place::PodmanHost(host.id))
            .chain(std::iter::once(farhelm_proto::Place::TensorlakeAccount))
            .collect::<Vec<_>>(),
        host_fixed,
    };
    let normalized = templates
        .iter()
        .cloned()
        .map(|mut template| {
            // Historical trust values are inert, even on an agent that did
            // not support them when the template was saved.
            template.fields.workspace_trust = None;
            remove_matching_held_fields(
                &mut template.fields,
                &state,
                draft.is_restart(),
                host_fixed,
            );
            template
        })
        .collect::<Vec<_>>();
    let mut repositories = Vec::with_capacity(normalized.len());
    for template in &normalized {
        if let Some(reason) = draft.restart_template_refusal(template) {
            return Err(reason);
        }
        state = farhelm_proto::launcher::apply_template(&state, template, &context)
            .map_err(|refusal| refusal.to_string())?;
        // Check this before the next template, so the first refusal in the
        // sequence is the one the launcher reports.
        repositories.push(match &template.fields.destination {
            Some(TemplateDestination::Github(repo)) => Some(
                crate::github_checkout::GithubRepo::parse(repo).map_err(|error| {
                    format!(
                        "Template {:?} was not applied: its destination {repo:?} is not a GitHub repository ({error})",
                        template.name
                    )
                })?,
            ),
            _ => None,
        });
    }
    Ok((state, repositories, normalized))
}

/// Keep UI-only interactions behind the live page token where they were gated.
///
/// HTML's disabled state is rendered asynchronously, while a create claims
/// the token synchronously before its key-mint await. Picker input, keyboard
/// routing and observation requests retain their UI checks here. Semantic edits
/// independently enter the owner's private gate and fixed retry policy.
fn draft_transition_allowed(ops: OpLock) -> bool {
    !ops.busy_now()
}

/// A queued automatic submit belongs to one opening, never a later draft.
/// The parent revokes the generation synchronously; the submit handler covers
/// the pre-render gap, and this selector covers a replaced or remounted form.
fn resubmit_quick_composer(generation: u64) {
    document::eval(&format!(
        "document.querySelector('.create-session-form[data-opening-generation=\"{generation}\"]')?.requestSubmit()"
    ));
}

/// The launcher's YOLO question: the key its [`ConfirmSlot`] is open for.
///
/// The question belongs to the launch the helm refused, named by that
/// request's intent key (`refused_key`). Which asking of it an answer was
/// drawn for is the slot's [`ConfirmToken`], so an answer drawn for an
/// earlier opening can never take a later one, even for the same refused
/// request. A YOLO answer records consent only by taking the opening its
/// handler was drawn for: Cancel followed by a YOLO answer in one event
/// burst (both drawn before the render that removes them) finds the slot
/// empty and records nothing. `ask` is what the question shows; the slot
/// holds nothing else.
///
/// `uncertain` marks a refused request whose key an earlier attempt already
/// carried with an unknown outcome (a lost reply, say): that attempt may have
/// started the session, so the question says so and offers no guarded answer,
/// which would mint a new request and could start a second one.
#[derive(Clone, PartialEq)]
struct LauncherYoloQuestion {
    refused_key: String,
    ask: crate::yolo_confirm::YoloAsk,
    uncertain: bool,
}

impl LauncherYoloQuestion {
    /// Whether "start guarded session instead" may be offered and taken.
    fn offers_guarded(&self, selection: Option<&LaunchSelection>) -> bool {
        !self.uncertain && self.ask.reason.can_launch_guarded(selection)
    }
}

/// Answer the launcher's YOLO question `question` (the opening this answer's
/// handler was drawn for) with "start anyway". When it is still open and the
/// launcher opening its render drew (`rendered_opening`) is still the live
/// one (`live_opening`): take it, record consent for its refused request in
/// `confirmed_key`, and for "don't ask again" (`stop_asking`) also in
/// `stop_asking_for`, and return `true`, after which the caller submits the
/// form. Otherwise change nothing and return `false`: a stale answer leaves
/// the form unauthorized and must not submit.
///
/// The launcher opening the answer's render drew must still be the live one:
/// opening another source swaps the launcher's prefill without unmounting it,
/// and nothing closes the question's slot on that swap, so an answer
/// delivered before the rerender would otherwise act on the earlier source,
/// which the user has already moved away from. The Guarded answer refuses the
/// same case (`take_launcher_guarded_answer`).
///
/// The one-off answer also clears `stop_asking_for`, so it can never mark the
/// host. Consent stays recorded under the refused key after the submit, which
/// is what lets a retry of the same request (a confirmed create whose reply
/// was lost) keep it; see `yolo_confirmed_key`.
fn answer_launcher_yolo(
    slot: &mut ConfirmSlot<LauncherYoloQuestion>,
    question: &ConfirmToken<LauncherYoloQuestion>,
    stop_asking: bool,
    rendered_opening: u64,
    live_opening: u64,
    confirmed_key: &mut Option<String>,
    stop_asking_for: &mut Option<ConfirmToken<LauncherYoloQuestion>>,
) -> bool {
    if rendered_opening != live_opening || slot.take(question).is_none() {
        return false;
    }
    *confirmed_key = Some(question.key().refused_key.clone());
    *stop_asking_for = stop_asking.then(|| question.clone());
    true
}

/// Bind a fresh preview to the accepted draft, even before a render publishes it.
/// Choice-and-Enter can apply a repository and submit in one event, so borrowing
/// the last rendered preview authority could capture a different destination.
fn checkout_preview_authority(
    host: &HostOption,
    draft: &LaunchDraft,
    config_revision: i64,
    agent: String,
) -> Option<PreviewAuthority> {
    Some(PreviewAuthority {
        generation: 0,
        config_revision,
        host: host.id.to_string(),
        incarnation: host.connection,
        installation_identity: host.identity.clone()?,
        repo: draft.destination().repo()?.clone(),
        title: Some(draft.effective_title()),
        agent,
    })
}

/// Bind deferred submission to the agent choices, including unfinished Custom text.
/// Field memos retain the preview's subscriptions; raw seeds come from its live owner.
fn preview_agent_identity(draft: LaunchDraftState) -> String {
    let f = draft.fields();
    let current = draft.snapshot();
    format!(
        "{:?}",
        (
            (
                (f.launch_tab)(),
                (f.structured_harness)(),
                (f.structured_model)()
            ),
            ((f.structured_effort)(), (f.structured_permissions)()),
            ((f.structured_permission_flags)(), (f.structured_wrapper)()),
            (f.container)(),
            submitted_field(
                &(f.invocation)(),
                (f.invocation_edited)(),
                current.invocation_raw_seed().as_deref(),
            ),
            (
                (f.command_yolo)(),
                (f.command_agent)(),
                (f.command_resume_on)()
            ),
            submitted_field(
                &(f.command_resume)(),
                (f.command_resume_edited)(),
                current.command_resume_raw_seed().as_deref(),
            ),
        )
    )
}

/// One accepted submission waiting only for its refreshed checkout preview.
///
/// Permissions participate in preview authority, so changing YOLO to guarded
/// deliberately replaces the preview generation. Ignore only that generation;
/// any other draft or opening change cancels the accepted automatic submit.
/// Resume additionally names the fresh connection; its predecessor is allowed
/// only while the independent host reader catches up, never for submission.
/// Automatic names may advance within the press's unchanged source policy;
/// typed and fixed names keep the exact-title comparison.
/// Resume also allows checkout configuration to be discovered or refreshed:
/// allocation belongs to the ready host's configuration, not its old epoch.
/// Prompt and conversation are launch inputs outside preview authority.
///
/// The opening and the Replace precondition are the ones the answered render
/// drew, never the values current at the click or at send time: the submit
/// that follows the preview sends `replace_guard` as captured, so a source
/// that became alive while the preview loaded is kept, as SPEC.md's Replace
/// with section requires.
#[derive(Debug, Clone)]
struct GuardedSubmitStamp {
    opening: u64,
    authority: PreviewAuthority,
    resuming_from: Option<u64>,
    resume_naming: Option<CheckoutNameSource>,
    prompt: Option<String>,
    conversation: String,
    replace_guard: crate::DeleteGuard,
    /// Internal preparation is not another answer and must not forget page input.
    press_baseline: Option<u64>,
}

/// One admitted submission keeps both exclusion owners through hidden preparation.
/// Replace's source claim settles before a returned dialog, while the launch
/// token retains that owed draft until admission. Keeping the optional source
/// with the existing continuation prevents a release/reclaim gap during wake.
struct SubmitGuards {
    launch: OpGuard,
    source: Option<(String, SessionGuard)>,
}

/// Preparation is over for the source even when its launcher cannot return yet.
/// Only real failures belong on the row; a question or a revoked empty draft can
/// release its claim without inventing one. The list refresh uses its ordinary
/// owner after the claim is dropped, independently of modal admission.
fn settle_prepared_source(
    source: &mut Option<(String, SessionGuard)>,
    reason: Option<&str>,
    mut operations: SessionOps,
    mut progress: crate::replace::ReplaceProgress,
    on_settled: EventHandler<()>,
) {
    if let Some((id, guard)) = source.take() {
        progress.0.write().remove(&id);
        if let Some(reason) = reason {
            operations
                .errors
                .write()
                .insert(id, format!("Replace: {reason}"));
        }
        drop(guard);
        on_settled.call(());
    }
}

impl GuardedSubmitStamp {
    /// Capture the launch's identity across the one expected preview refresh.
    /// The caller supplies the post-transition agent fingerprint, even before
    /// a render has installed that fingerprint in live preview authority.
    fn new(
        opening: u64,
        mut authority: PreviewAuthority,
        prompt: Option<String>,
        conversation: String,
        replace_guard: crate::DeleteGuard,
        press_baseline: Option<u64>,
    ) -> Self {
        authority.generation = 0;
        Self {
            opening,
            authority,
            resuming_from: None,
            resume_naming: None,
            prompt,
            conversation,
            replace_guard,
            press_baseline,
        }
    }

    /// Wait across the display handoff, then accept only Resume's connection.
    /// The old token cannot authorize even a Ready preview. Any third token
    /// cancels the press instead of following another reconnection implicitly.
    fn after_resume(mut self, connection: u64, naming: Option<CheckoutNameSource>) -> Self {
        self.resuming_from = Some(self.authority.incarnation);
        self.authority.incarnation = connection;
        self.resume_naming = naming;
        self
    }

    /// Whether the form still sees the connection Resume just replaced.
    /// When Resume preserves a connection, no display handoff is needed.
    fn waiting_for_connection(&self, authority: &PreviewAuthority) -> bool {
        self.resuming_from == Some(authority.incarnation)
            && authority.incarnation != self.authority.incarnation
    }

    /// Whether a press parked after a successful wake is still waiting for this
    /// window's own host list to show the woken host connected: the form still
    /// sees the connection the wake replaced, or the selected host still shows
    /// a connection pending. This is the only wait [`WOKEN_CONNECTION_WAIT_MS`]
    /// bounds. Once the window sees the connection, the press waits for its
    /// checkout preview instead, which keeps its own request budget.
    ///
    /// The caller passes both readings evaluated, so an effect using this
    /// subscribes to both.
    fn awaiting_woken_connection(
        &self,
        authority: Option<&PreviewAuthority>,
        resume_display: Option<&ResumeDisplayHandoff>,
    ) -> bool {
        self.resuming_from.is_some()
            && (authority.is_some_and(|authority| self.waiting_for_connection(authority))
                || resume_display.is_some())
    }

    /// The precondition to send when the live launch is still the one this
    /// answer or press accepted, or `None` when its identity changed. A match
    /// during the old connection's display handoff still requires waiting:
    /// callers must check `waiting_for_connection` before using the guard.
    /// The captured precondition is never compared: it is what the submit
    /// sends, and the live one moving (a source restarted meanwhile) must not
    /// turn the accepted launch into a different deletion.
    fn guard_if_current(
        &self,
        opening: u64,
        mut authority: PreviewAuthority,
        prompt: &Option<String>,
        conversation: &str,
        naming: Option<&CheckoutNameSource>,
    ) -> Option<crate::DeleteGuard> {
        authority.generation = 0;
        if self.waiting_for_connection(&authority) {
            authority.incarnation = self.authority.incarnation;
        }
        if self.resuming_from.is_some() {
            // A shortcut can press before this host's history reveals its
            // checkout epoch. The press accepts allocation after Resume; only
            // the form's current Ready preview, checked in full at dispatch,
            // authorizes a directory under that configuration.
            authority.config_revision = self.authority.config_revision;
        }
        // The press accepts allocation from an automatic name series, not its
        // pre-suspension candidate. Only the unchanged source policy may advance;
        // typing a name retires that source and cancels the accepted press.
        if let Some(accepted) = &self.resume_naming {
            if Some(accepted) != naming {
                return None;
            }
            authority.title.clone_from(&self.authority.title);
        }
        (self.opening == opening
            && self.authority == authority
            && &self.prompt == prompt
            && self.conversation == conversation)
            .then_some(self.replace_guard)
    }
}

/// Whether the parked slot holds a woken press that still owns its launch
/// token and still waits for the woken connection: what the 60-second limit
/// may discard, and what the form's diagnostic attribute reports. One helper
/// for both, so a test reading the attribute observes the timer's own rule.
/// `parked` is the slot's stamp and whether it retains its token; a visible
/// guarded answer retains none and is never this wait.
fn parked_press_awaiting_woken_connection(
    parked: Option<(&GuardedSubmitStamp, bool)>,
    authority: Option<&PreviewAuthority>,
    resume_display: Option<&ResumeDisplayHandoff>,
) -> bool {
    parked.is_some_and(|(stamp, retains_token)| {
        retains_token && stamp.awaiting_woken_connection(authority, resume_display)
    })
}

/// Take only an answer for the still-visible refused launch, before any edit.
///
/// Slot identity rejects Cancel and older openings. Binding equality rejects
/// an edit that reached live signals before the render removed its old button.
/// The launcher opening the answer's render drew must still be the live one:
/// opening another source swaps the launcher's prefill without unmounting it,
/// and an answer delivered before the rerender would otherwise act on the
/// earlier source. A busy operation leaves the question intact, including its
/// error.
fn take_launcher_guarded_answer(
    slot: &mut ConfirmSlot<LauncherYoloQuestion>,
    question: &ConfirmToken<LauncherYoloQuestion>,
    busy: bool,
    rendered_opening: u64,
    live_opening: u64,
    refused: &IntentBinding,
    current: Option<&IntentBinding>,
) -> bool {
    if busy || rendered_opening != live_opening || current != Some(refused) {
        return false;
    }
    let selection = match &refused.agent {
        LaunchIntent::Structured(selection) => Some(selection),
        LaunchIntent::Command(_) => None,
    };
    question.key().offers_guarded(selection) && slot.take(question).is_some()
}

/// Reuse the refused checkout's accepted facts when its semantic intent matches.
/// This is a projection through the sole request builder, not preview admission:
/// callers retain their own authority checks and observation/read ordering.
fn bind_launcher_question_checkout(
    current: &LaunchDraft,
    candidate: IntentBinding,
    repo: &GithubRepo,
    installation: &str,
    refused: &IntentBinding,
) -> Option<IntentBinding> {
    if !same_fresh_intent(refused, &candidate, repo, installation) {
        return None;
    }
    current.request(RequestContext {
        host: Some((candidate.host, &candidate.incarnation)),
        launch: candidate.agent.clone(),
        replace_source: candidate.replace_source.as_deref(),
        checkout: Some(CheckoutInput::Accepted {
            cwd: &refused.cwd,
            request: refused.github_checkout.as_ref()?,
        }),
    })
}

/// Identify a completed template without subscribing to its caller's effect.
/// Templates retain the prior retry pair until submit compares the final draft.
/// Its question instead belongs only to that unchanged completed operation.
/// A managed question retains the refused checkout even while a new preview is
/// pending; this read grants no permission to submit or answer the question.
fn completed_template_question_binding(
    current: &LaunchDraft,
    hosts: &[HostOption],
    selected: Option<HostId>,
    target: Option<&CreateTarget>,
    replace_source: Option<&str>,
    refused: &IntentBinding,
) -> Option<IntentBinding> {
    let launch = match current.tab() {
        LaunchTab::Agent => {
            current.harness()?;
            wrapper_words(&current.wrapper()).ok()?;
            current.launch_intent()?
        }
        // Dormant Agent editors cannot invalidate an unchanged Command.
        LaunchTab::Command => current.launch_intent()?,
    };
    if !target_matches_selection(selected, hosts, target) {
        return None;
    }
    let host = hosts.iter().find(|host| Some(host.id) == selected)?;
    let candidate = current.request(RequestContext {
        host: Some((host.id, &host.incarnation)),
        launch,
        replace_source,
        checkout: None,
    })?;
    match current.destination() {
        DestinationDraft::Existing { .. } => Some(candidate),
        DestinationDraft::GithubInput { .. } => None,
        DestinationDraft::Github { repo, .. } => bind_launcher_question_checkout(
            current,
            candidate,
            &repo,
            host.identity.as_deref()?,
            refused,
        ),
    }
}

/// End the actual opening when a completed template no longer describes it.
/// Keep the restored retry pair: submit, not template replay, decides whether
/// its request needs another key. Exact cancellation also leaves a newer opening
/// alone, so returning to old choices cannot resurrect a consumed question.
fn retire_changed_template_question(
    slot: &mut ConfirmSlot<LauncherYoloQuestion>,
    retained: Option<&(String, IntentBinding)>,
    current: Option<&IntentBinding>,
) -> bool {
    let Some(question) = slot.peek_key() else {
        return false;
    };
    if retained
        .is_some_and(|(key, refused)| key == &question.refused_key && current == Some(refused))
    {
        return false;
    }
    // Reconciliation, not an answer: this one synchronous call both read and
    // closes the opening, so nothing newer can have replaced it in between.
    slot.clear();
    true
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
///   `hosts::host_incarnation`. Fresh checkout retries additionally retain
///   the accepted installation identity and exact request body: authenticated
///   lookup may reconcile that original intent across a new incarnation on
///   the same installation. [`same_fresh_intent`] is that narrow exception;
///   a different installation must never receive the old attempt's key.
/// - **The submitted values, snapshotted.** Ordinary edits retire the key
///   when those values change; equal text or choices still update ownership
///   without abandoning an unresolved retry. That eager rule has a gap the
///   size of one await: minting is asynchronous and the
///   `disabled` attributes that make the form inert land one render after the
///   submit, so a keystroke queued at submit time can change a field while the
///   key is being made. The binding is re-read after minting and compared
///   against this, which turns that gap into another mint rather than a key
///   that describes something the user did not submit — the attributes are
///   honesty about a create being in flight, not the guard.
///
/// LaunchDraft::request is the one envelope assembler. Admission stays with the
/// form: a preliminary binding can exist before a repository preview is usable,
/// while a retained managed attempt keeps its original binding and wire body.
#[derive(Debug, Clone, PartialEq, Eq)]
struct IntentBinding {
    /// One-time first message; retries bind its exact bytes, never saved history.
    prompt: Option<String>,
    /// Transient choice bound to this request/key, absent from saved launch history.
    conversation: Option<String>,
    host: HostId,
    /// The target host's incarnation at submit time — see the type docs.
    incarnation: String,
    cwd: String,
    /// Fresh requests carry the exact accepted preview as part of the key's
    /// identity. Existing requests leave this absent and keep their old body.
    github_checkout: Option<GithubCheckoutRequest>,
    /// What this create launches — see [`LaunchIntent`]. Switching between
    /// the two modes is a different intended create, which is why the mode
    /// lives inside the binding rather than beside it.
    agent: LaunchIntent,
    /// Ordinary blank names share the API's omitted-title identity. Managed
    /// names remain exact because accepted preview authority binds their bytes.
    title: String,
    /// `Some(source id)` when this binding is a "replace with", carried
    /// projected from the mounted [`CreatePrefill::mode`] — `None`
    /// for every ordinary create or clone. Part of the binding, not a
    /// side channel next to it, for the same idempotency reason every other
    /// field here is: a retried replace-with must reuse its own key
    /// (matching this same source id), and a plain create must never be
    /// able to collide with one — which `PartialEq`/`Eq` on the whole
    /// struct give for free the moment this field exists, with no bespoke
    /// comparison to keep in sync. The submit handler reads it after
    /// minting to decide whether to call `api::replace_session_with`
    /// instead of `api::create_session` (see that call site's own
    /// comment).
    replace_source: Option<String>,
}

/// Reconciliation follows user intent and installation, not the latest config
/// or connection counter. The retained body still carries its original preview
/// and incarnation for the helm's authenticated reconciliation path.
/// Installation matching uses the original reply bytes, independently of how
/// an identity might be presented to the user.
fn same_fresh_intent(
    accepted: &IntentBinding,
    current: &IntentBinding,
    repo: &GithubRepo,
    installation: &str,
) -> bool {
    accepted.host == current.host
        && accepted.prompt == current.prompt
        && accepted.agent == current.agent
        && accepted.title == current.title
        && accepted.replace_source == current.replace_source
        && accepted.github_checkout.as_ref().is_some_and(|checkout| {
            checkout.repo == repo.identifier()
                && checkout.preview.installation_identity.raw() == installation
        })
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

/// Interleave bounded histories without changing either source's relative order.
/// Ordinary entries retain the supervisor's sequence ordering, even across a
/// backward wall-clock jump. Environment entries retain helm admission order.
/// Timestamps choose only between the current heads; a tie favors ordinary
/// history, and neither source's independent sequence is compared to the other.
fn merge_recent_launches(
    ordinary: Vec<api::LaunchHistoryEntry>,
    environment: &[api::LaunchHistoryEntry],
) -> Vec<api::LaunchHistoryEntry> {
    let mut merged = Vec::with_capacity(ordinary.len() + environment.len());
    let mut ordinary = ordinary.into_iter().peekable();
    let mut environment = environment.iter().cloned().peekable();
    while let (Some(ordinary_head), Some(environment_head)) = (ordinary.peek(), environment.peek())
    {
        if environment_head.created_at > ordinary_head.created_at {
            merged.push(environment.next().expect("environment head was observed"));
        } else {
            merged.push(ordinary.next().expect("ordinary head was observed"));
        }
    }
    merged.extend(ordinary);
    merged.extend(environment);
    merged
}

/// Project suggestions without granting environment history ordinary-folder
/// authority. Ordinary offers need both live installation checks; helm-wide
/// environment intent remains available even when no ordinary target exists.
/// Search's immediate active-index calculation must use this same projection.
fn offered_recent_history(
    ordinary: Option<&(CreateTarget, api::LaunchHistory)>,
    current: Option<&CreateTarget>,
    destination: Option<&CreateTarget>,
    environment: &[api::LaunchHistoryEntry],
) -> api::LaunchHistory {
    let mut history = ordinary
        .and_then(|(target, history)| {
            (Some(target) == current && Some(target) == destination).then(|| history.clone())
        })
        .unwrap_or_default();
    history.launches = merge_recent_launches(history.launches, environment);
    history
}

/// Hand successful environment observations to deliberate history promotion.
/// Pending or failed reads retain the last success, and a successful empty
/// first offer stays frozen just like a nonempty one. This establishes no
/// authority for an ordinary host's folder or checkout configuration.
fn observe_environment_history(
    answer: Option<&Result<Vec<api::LaunchHistoryEntry>, String>>,
    mut fetched: Signal<Option<Vec<api::LaunchHistoryEntry>>>,
    mut offered: Signal<Option<Vec<api::LaunchHistoryEntry>>>,
    mut revision: Signal<u64>,
) {
    if let Some(Ok(entries)) = answer
        && fetched.peek().as_ref() != Some(entries)
    {
        fetched.set(Some(entries.clone()));
        revision.with_mut(|revision| *revision = revision.wrapping_add(1));
    }
    if offered.peek().is_none()
        && let Some(entries) = fetched.peek().as_ref()
    {
        offered.set(Some(entries.clone()));
    }
}

/// Where a launcher draft stands on its default folder (SPEC.md, "Working
/// directory"): whether a host's recent setups still have to seed it, which
/// target and Clone/Replace source the current default belongs to, and
/// whether the last seed fell back to home because those setups could not be
/// read (a hidden quick launch must not launch into that guess).
#[derive(Debug, Clone, Default, PartialEq)]
struct FolderDefault {
    pending: bool,
    target: Option<CreateTarget>,
    prefill: Option<u64>,
    unread: bool,
    /// The folder decided for `target`, kept apart from the recent-setups
    /// snapshot, which a later promotion can replace.
    decided: Option<String>,
}

/// What `folder_default_step` reads besides its own state, all for one render.
struct FolderDefaultInputs<'a> {
    target: Option<&'a CreateTarget>,
    prefill: Option<u64>,
    /// Whether a target change is the person's (or a template's) choice
    /// rather than a Clone's automatic binding to its source host.
    host_is_users: bool,
    /// The selected session's installation and folder, which outrank history.
    selected: Option<&'a (CreateTarget, String)>,
    /// The first recent-setups reply for its target.
    offered: Option<&'a (CreateTarget, api::LaunchHistory)>,
    /// A target whose recent-setups read failed.
    unreadable: Option<&'a CreateTarget>,
    /// The state captured with a permission-switch snapshot, when that
    /// snapshot is being restored (permission Undo) in this render.
    restored: Option<&'a FolderDefault>,
}

/// A host's remembered folder: the newest recent setup that launched into a
/// plain folder (fresh managed checkouts do not count), else `~`, which the
/// host expands to its own home directory.
fn remembered_folder(history: &api::LaunchHistory) -> &str {
    history
        .launches
        .iter()
        .find(|launch| launch.github_repo.is_none())
        .map_or("~", |launch| launch.cwd.raw())
}

/// Advance the folder default by one render, returning the folder to seed.
///
/// A Clone/Replace source owns the folder of the target it was applied on, and
/// an opening's inherited session folder starts the state not pending. The
/// first target an opening resolves is not a host change; a later target
/// change re-arms the default when `host_is_users` says the person, not a
/// Clone's automatic binding to its source host, chose it (the picker, a
/// template's host or local home). A pending default then follows SPEC.md's
/// precedence for the target: the selected session's folder on its own
/// installation, else the host's remembered folder from `offered`, the first
/// recent-setups reply for the target (so a feed refresh of the same history
/// never rewrites an open draft's folder). An unreadable history falls back to
/// home rather than leaving the previous host's folder in place, and marks the
/// default `unread`. A permission Undo restores the draft as it was at the
/// switch, so it also restores this state as captured then: a folder decided
/// then is applied again (which also retires a restored installation claim),
/// a settled source or inherited folder is left alone, and a default that was
/// still pending takes the decision made since for the same target, if any.
/// Whether the draft's folder is still a default is the draft's own check at
/// seeding time.
fn folder_default_step(
    mut state: FolderDefault,
    inputs: FolderDefaultInputs<'_>,
) -> (FolderDefault, Option<String>) {
    let FolderDefaultInputs {
        target,
        prefill,
        host_is_users,
        selected,
        offered,
        unreadable,
        restored,
    } = inputs;
    let mut restore_seed = None;
    if let Some(snapshot) = restored {
        // The decision already made for the restored destination, if any.
        let live = (state.target.as_ref() == target)
            .then(|| state.decided.clone().map(|folder| (folder, state.unread)))
            .flatten();
        let mut next = snapshot.clone();
        if snapshot.target.as_ref() == target {
            // The snapshot describes this destination: its decision stands,
            // a settled source or inherited folder stays, and a default still
            // pending then takes the decision made since, if any.
            if snapshot.pending
                && let Some((folder, unread)) = live
            {
                next.pending = false;
                next.decided = Some(folder);
                next.unread = unread;
            }
        } else if !host_is_users {
            // A Clone/Replace source still owns placement (its host binding was
            // withdrawn automatically): its settled folder stays.
            next.target = target.cloned();
            next.pending = false;
            next.decided = None;
        } else {
            // Captured before its own target was decided (a template batch's
            // host change, or an installation replaced since): what was
            // already decided for the restored destination stands.
            next.target = target.cloned();
            match live {
                Some((folder, unread)) => {
                    next.pending = false;
                    next.decided = Some(folder);
                    next.unread = unread;
                }
                None => {
                    next.pending = true;
                    next.decided = None;
                    next.unread = false;
                }
            }
        }
        restore_seed = next.decided.clone();
        state = next;
    }
    let target_before = state.target.clone();
    if state.prefill != prefill {
        state.prefill = prefill;
        state.pending = false;
        state.unread = false;
        state.decided = None;
        state.target = target.cloned();
    } else if state.target.as_ref() != target {
        // A decision belongs to one target; a source-owned move must not
        // relabel it as the new target's.
        state.decided = None;
        if state.target.is_some() && host_is_users {
            state.pending = true;
            state.unread = false;
        }
        state.target = target.cloned();
    }
    if !state.pending || target.is_none() {
        // A restore's decision stands only for the target it was made for.
        let seed = restore_seed.filter(|_| state.target == target_before);
        return (state, seed);
    }
    let seed = match (selected, offered) {
        (Some((selected_target, cwd)), _) if Some(selected_target) == target => Some(cwd.clone()),
        (_, Some((offered_target, history))) if Some(offered_target) == target => {
            Some(remembered_folder(history).to_owned())
        }
        _ if unreadable == target => {
            state.unread = true;
            Some("~".to_owned())
        }
        _ => None,
    };
    if seed.is_some() {
        state.pending = false;
        state.decided.clone_from(&seed);
    }
    (state, seed)
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

/// What a host's answer says is wrong with a launcher's folder, when it says so.
///
/// Recognized from the supervisor's fixed refusal words (`ensure_cwd_usable`
/// and its siblings in `farhelm-supervisor`'s `service/core.rs`) by
/// "contains", because helm paths can wrap the supervisor's sentence in
/// their own. Matching text is acceptable here, unlike for consent or retry
/// decisions, which read structured headers only: the marking it drives
/// authorizes nothing and only points at the field the refusal is about. If
/// the words ever change, the launcher degrades to the refusal alert alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FolderProblem {
    Missing,
    NotADirectory,
    Unusable,
}

impl FolderProblem {
    fn recognize(message: &str) -> Option<FolderProblem> {
        [
            ("working directory does not exist: ", FolderProblem::Missing),
            (
                "working directory is not a directory: ",
                FolderProblem::NotADirectory,
            ),
            ("working directory is not usable: ", FolderProblem::Unusable),
        ]
        .into_iter()
        .find(|(words, _)| message.contains(words))
        .map(|(_, problem)| problem)
    }

    /// The line under the folder field, around the host's peer-supplied name.
    fn notice(self, host: &str) -> Vec<DetailPart> {
        let lead = match self {
            FolderProblem::Missing => "⚠ This folder does not exist on ",
            FolderProblem::NotADirectory => "⚠ This path is not a folder on ",
            FolderProblem::Unusable => "⚠ This folder cannot be used on ",
        };
        vec![
            DetailPart::text(lead),
            DetailPart::peer(host.to_owned()),
            DetailPart::text("."),
        ]
    }
}

/// A folder refusal and the destination it was about. The field marking is
/// derived from it on every render and shows only while the form still names
/// the same installation, connection and raw folder, so any change of host,
/// reconnect or edit hides it without each handler having to clear it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FolderRefusal {
    target: CreateTarget,
    connection: Option<u64>,
    cwd: String,
    host: String,
    problem: FolderProblem,
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

/// Keep history and its configuration epoch current after a feed notice.
/// A failed request must retain demand: otherwise one lost response consumes
/// the only notification of a CLI edit and leaves an open preview stale until
/// an unrelated fleet change. The shared reader bounds concurrency and retries
/// and withdraws unattended work under build skew. Results keep their target
/// identity, so a host switch cannot borrow its predecessor's suggestions.
fn request_launch_history(
    base: String,
    target: Signal<Option<CreateTarget>>,
    reader: Signal<SurfaceReader>,
    mut result: Signal<Option<(CreateTarget, api::LaunchHistory)>>,
    mut unreadable: Signal<Option<CreateTarget>>,
    trigger: Trigger,
) {
    request_read(reader, trigger, move || {
        let base = base.clone();
        let requested = target.peek().clone();
        async move {
            let Some(requested) = requested else {
                result.set(None);
                return true;
            };
            match api::fetch_launch_history(&base, requested.host).await {
                Ok(history) => {
                    if target.peek().as_ref() == Some(&requested) {
                        unreadable.set(None);
                        result.set(Some((requested, history)));
                    }
                    true
                }
                // The reader still retries, but a launcher waiting on this
                // target's default folder must learn that it failed.
                Err(_) => {
                    if target.peek().as_ref() == Some(&requested) {
                        unreadable.set(Some(requested));
                    }
                    false
                }
            }
        }
    });
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
/// The caller supplies the live working draft, never the folder memo: a queued
/// activation may follow a raw-only reseed whose display and edited bit match.
/// Keep request, target and connection observations tracked and in this order.
fn browse_activation_is_current(
    request: Signal<Option<BrowseAuthority>>,
    candidate: &BrowseAuthority,
    target: Signal<Option<CreateTarget>>,
    connection: Signal<Option<u64>>,
    draft: &LaunchDraft,
) -> bool {
    browse_reply_is_current(
        &request(),
        candidate,
        target(),
        connection(),
        &draft.submitted_cwd(),
    )
}

/// Start one directory listing bound to the current create destination.
///
/// Both the ordinary Browse button and search's Browse-this-path action use
/// this entry point so neither can accidentally weaken the target,
/// connection, path, or generation guard. The request is explicit: callers
/// choose when to invoke it; merely updating the search query never reaches
/// the remote filesystem. Completion reads the live owner's operational path;
/// retaining a render snapshot would admit replies after a same-turn edit.
#[expect(
    clippy::too_many_arguments,
    reason = "browse authority and live draft ownership are independent completion fences"
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
    draft: LaunchDraftState,
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
            &draft.snapshot().submitted_cwd(),
        ) {
            return;
        }
        match result {
            Ok(result) => browse_result.set(Some((authority, result))),
            Err(reason) => browse_error.set(Some(reason)),
        }
    });
}

/// Address the same form from its shell and the list-owned completion callbacks.
pub(super) const COMPOSER_DIALOG: DialogHandle =
    DialogHandle::new(".create-session-form[role=\"dialog\"]");

/// Escape belongs to the inline template panel while it is open. Its disabled
/// Cancel also keeps a pending save uncancellable; document order alone would
/// otherwise select the outer launcher's Cancel first.
const COMPOSER_ESCAPE_TARGET: &str = ".save-template-cancel, .launch-composer-cancel:not([data-template-panel-open]), .launch-composer-loading-cancel";

/// Give page input back before hiding this submitted opening's focused control.
/// The form and its request future stay mounted; only modal isolation retires.
/// Arm restoration even if a fast reply settles before the hidden render commits.
fn release_submitted_composer(opening: u64, mut previous_presentation: Signal<(bool, bool)>) {
    previous_presentation.set((true, true));
    COMPOSER_DIALOG
        .for_generation("data-opening-generation", opening)
        .release();
}

/// Hand the keyboard to the page once a hidden launcher has stopped rendering.
///
/// Chromium moves focus off a control that stops rendering; WebKit sometimes
/// leaves it on the hidden Launch button (or the form root), so later keys go
/// to a control nobody can see and the two engines disagree about where a
/// repeated Enter lands. The presentation effect calls this after the hidden
/// render is in the DOM. Blurring any earlier, while the form still shows,
/// would let a quick second Enter reach the page before `launch-activity.js`
/// can recognize it as the same submit repeated. A focused control that still
/// renders keeps its focus, so repeated calls while hidden change nothing.
fn blur_hidden_composer(opening: u64) {
    document::eval(&format!(
        r#"(() => {{
            const form = document.querySelector('.create-session-form[data-opening-generation="{opening}"]');
            const active = document.activeElement;
            if (form && active && form.contains(active) && !active.getClientRects().length) active.blur();
        }})();"#
    ));
}

/// Keep the submitted owner until its returning dialog can claim page input.
/// Navigation can open another modal during a request. Neither dialog may
/// isolate the other, so the existing request state and guard stay held until
/// that modal leaves. The captured root and opening fence retire the observer
/// on cancellation instead of admitting a replacement launcher. Claiming the
/// ordinary modal marker before returning closes the eval-to-render gap, also
/// against a host result that reserved admission before rendering its dialog.
/// Naming alone supplies a known press baseline: the same script rejects changed
/// input before reserving modality, and the existing ordered relay can abandon a
/// blocked naming wait. Failure/question Returns remain owed regardless of input.
async fn admit_composer_return(opening: u64, naming_baseline: Option<u64>) -> bool {
    let baseline = serde_json::to_string(&naming_baseline).expect("an optional count serializes");
    let result = document::eval(&format!(
        r#"return await new Promise(resolve => {{
            const dialog = document.querySelector('.create-session-form[role="dialog"]');
            const baseline = {baseline};
            const observer = new MutationObserver(admit);
            function finish(accepted) {{
                observer.disconnect();
                document.removeEventListener('input', observeActivity, true);
                resolve(accepted);
            }}
            function observeActivity(event) {{
                if (event.target?.matches?.('input[data-launch-activity]')) admit();
            }}
            function admit() {{
                if (!dialog?.isConnected ||
                    dialog.getAttribute('data-opening-generation') !== '{opening}') {{
                    finish(false); return;
                }}
                // Count publication precedes its relay event. A naming wait
                // loses authority on input before it reserves any modal marker.
                if (baseline !== null && window.__farhelmLaunchActivity?.() !== baseline) {{
                    finish(false); return;
                }}
                const blocked = Array.from(document.querySelectorAll(
                    '[role="dialog"][aria-modal="true"], [data-host-modal-admitted]'
                )).some(node => node !== dialog);
                if (blocked) return;
                dialog.setAttribute('aria-modal', 'true');
                finish(true);
            }}
            observer.observe(document.body, {{ childList: true, subtree: true,
                attributes: true, attributeFilter: ['aria-modal', 'data-host-modal-admitted', 'data-opening-generation'] }});
            if (baseline !== null) document.addEventListener('input', observeActivity, true);
            admit();
        }});"#
    ))
    .join::<bool>()
    .await;
    // A failed eval must not discard an error/question result. Naming has no
    // such obligation: failed admission evidence cannot authorize that dialog.
    if naming_baseline.is_some() {
        matches!(result, Ok(true))
    } else {
        !matches!(result, Ok(false))
    }
}

/// A failed hidden preparation retains its draft and token until Return is admitted.
/// Wake and preview can finish before any create exists. Their failure is still
/// owed back to this opening, and releasing the token first would let New replace
/// it while another dialog owns input. A retired opening releases only its own seed.
async fn return_prepared_composer(
    opening: u64,
    mut resuming_host: Signal<Option<HostResumePreparation>>,
    on_retired: EventHandler<u64>,
    _guard: OpGuard,
) {
    let admitted = admit_composer_return(opening, None).await;
    resuming_host.set(None);
    if !admitted {
        on_retired.call(opening);
    }
}

/// Naming belongs only to the accepted launch answer the user is still awaiting.
/// The page count is read at the reply and after an admitted naming wait: input
/// during either preparation or another modal must skip that extra dialog. A
/// retired owner still reports successful creation to the list, without naming.
async fn completed_launch_input(
    opening: u64,
    generation: Signal<u64>,
    baseline: Option<u64>,
    activity: LaunchActivity,
    may_name: bool,
) -> (Option<u64>, bool) {
    let mut current = activity.current().await;
    let still_awaited = |current| {
        launch_is_current(Some(*generation.peek()), opening, baseline, current)
            && baseline == activity.snapshot()
    };
    let mut naming_allowed = still_awaited(current);
    if may_name && naming_allowed {
        let admitted = admit_composer_return(opening, baseline).await;
        current = activity.current().await;
        naming_allowed = admitted && still_awaited(current);
        if !admitted {
            // The result is still reported, but a retired naming admission
            // cannot carry navigation authority into the parent's callback.
            current = None;
        }
    }
    if *generation.peek() != opening {
        current = None;
    }
    (current, naming_allowed)
}

/// Restore modality when a submitted wait returns to preparation or user input.
/// Hidden quick preparation regains isolation without focusing hidden fields.
/// A new opening or another submit must not receive stale frame work. Existing
/// question focus takes priority when a refusal reveals YOLO. Ordinary launches
/// return to search; Restart returns to its primary action after a refusal.
fn restore_composer_modal(opening: u64, focus_surface: bool, target: FocusTarget) {
    let isolate = COMPOSER_DIALOG
        .for_generation("data-opening-generation", opening)
        .isolation_script(Some(COMPOSER_ESCAPE_TARGET));
    let focus = target.surface_script();
    document::eval(&format!(
        r#"requestAnimationFrame(() => {{
            const dialog = document.querySelector('.create-session-form[role="dialog"]');
            if (!dialog?.isConnected ||
                dialog.getAttribute('data-opening-generation') !== '{opening}' ||
                dialog.getAttribute('aria-modal') !== 'true') return;
            const active = document.activeElement;
            const ownsFocus = active && active !== dialog && dialog.contains(active) &&
                active.getClientRects().length && !active.closest('[inert]') &&
                !active.matches(':disabled, [aria-disabled="true"]');
            if ({focus_surface} && dialog.getClientRects().length && !ownsFocus) {{ {focus} }}
            {isolate}
        }})"#
    ));
}

/// Search is the shared target; loading keeps Cancel as a keyboard exit.
/// The shell supplies the visible-root check and final container fallback.
fn composer_surface_target() -> FocusTarget {
    FocusTarget::new(
        ".launch-composer-search input:not([disabled])",
        Some(".launch-composer-loading-cancel:not([disabled])"),
        false,
    )
}

/// Give the shared composer search focus at an explicit interaction boundary.
///
/// Mode changes and accepted search results focus immediately so queued frame
/// work cannot steal later input. Reveal uses `restore_composer_modal` instead;
/// ordinary rerenders never reclaim a field the user deliberately entered.
/// Loading may still have only Cancel mounted, which remains a keyboard exit.
fn focus_composer_surface() {
    COMPOSER_DIALOG.focus_surface(&composer_surface_target(), false);
}

/// Restore a launcher focus target only when a shortcut removed its control.
/// A surviving field keeps its caret and selection; a later dialog opening or
/// launcher replacement must not receive focus from this deferred check.
/// Capture the root before the frame; the shared target script uses that node
/// only after its connection, visibility and ownership guards have admitted it.
fn restore_shortcut_focus_if_lost() {
    let focus = composer_surface_target().surface_script();
    document::eval(&format!(
        r#"(() => {{
            const dialog = document.querySelector('.create-session-form[role="dialog"]');
            requestAnimationFrame(() => {{
                if (!dialog?.isConnected || !dialog.getClientRects().length ||
                    window.farhelmTemplateShortcutContext?.() !== 'launcher') return;
                const active = document.activeElement;
                if (active && active !== document.body && active !== document.documentElement) return;
                {focus}
            }});
        }})()"#,
    ));
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

/// Keep the active folder row visible while focus remains on the combobox.
/// Called by the post-render effect so a newly opened list is already mounted.
fn scroll_folder_result(index: usize) {
    document::eval(&format!(
        r#"document.getElementById('launcher-folder-{index}')?.scrollIntoView({{ block: 'nearest' }});"#,
    ));
}

/// Keep the active repository visible without moving focus off its combobox.
/// The post-render caller also covers a list mounted by the current render.
fn scroll_repository_result(index: usize) {
    document::eval(&format!(
        r#"document.getElementById('checkout-repository-{index}')?.scrollIntoView({{ block: 'nearest' }});"#,
    ));
}

/// What the launcher carries from its source and which operation it submits.
///
/// Clone uses `Create`, since its source only seeds a new session. Replace
/// seeds the source folder, holds its host and addresses the replace endpoint.
/// Restart additionally holds kind, agent and folder, and compares edits with
/// the opening launch. These source facts are operation data, not draft fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LauncherMode {
    Create,
    Replace {
        source: String,
        opened: (SessionStatus, usize),
    },
    Restart {
        /// The existing session; restart never creates a replacement identity.
        source: String,
        /// Stored-command semantics, with historical permission omissions kept.
        baseline: api::RestartEdit,
        /// Exact admission snapshot, including fields absent from the baseline.
        /// Out-of-line storage keeps the small Create/Replace modes compact.
        opened_launch: Option<Box<crate::SessionLaunch>>,
        conversation_required: bool,
    },
}

impl LauncherMode {
    /// The session the replace endpoint addresses, absent for create or restart.
    ///
    /// Retry bindings carry this value, so a create and a replace with
    /// identical launch settings still mint different retry keys.
    pub(super) fn replace_source(&self) -> Option<&str> {
        match self {
            Self::Create | Self::Restart { .. } => None,
            Self::Replace { source, .. } => Some(source),
        }
    }

    /// Whether templates must preserve the source host instead of setting one.
    /// Clone can choose another host; Replace and Restart address their source.
    pub(super) fn holds_host(&self) -> bool {
        match self {
            Self::Create => false,
            Self::Replace { .. } | Self::Restart { .. } => true,
        }
    }

    /// Whether seeding starts in the source folder rather than a fresh checkout.
    /// This is a seeding rule: Replace can still edit its destination afterward.
    fn seeds_source_folder(&self) -> bool {
        match self {
            Self::Create => false,
            Self::Replace { .. } | Self::Restart { .. } => true,
        }
    }

    /// Opening state used only until a listing has observed this source.
    ///
    /// The live listing wins, then its last observation, then this snapshot
    /// for a source hidden by a filter or awaiting its first listing. The
    /// warning and destructive precondition share that one fallback; a failed
    /// read must not quietly return them to an older opening state.
    pub(super) fn source_opened(&self) -> Option<&(SessionStatus, usize)> {
        match self {
            Self::Create | Self::Restart { .. } => None,
            Self::Replace { opened, .. } => Some(opened),
        }
    }

    /// Restart retains its existing identity and all source placement fields.
    pub(super) fn is_restart(&self) -> bool {
        matches!(self, Self::Restart { .. })
    }
}

/// What one "clone" click seeds a fresh create form with: everything about
/// the clicked row that a NEW session can reuse.
///
/// Built once, by [`prefill_from`], from the row's `Session` at the moment
/// of the click — a snapshot, not a live binding: a row that changes after
/// the click must not reach back into an open, already-prefilled form.
///
/// `title` and `cwd` retain the source's spelling for an existing-folder
/// destination. Checkout membership separately seeds Clone onto a fresh
/// checkout with an untouched `-clone` name; Replace with ignores that repo
/// and starts from the existing folder. The raw folder remains available
/// when the person chooses an existing-folder destination instead. Folder
/// titles remain verbatim, duplicates allowed: selecting Clone does not
/// request a title edit for work in an existing directory.
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
    /// accepts from such a peer. Restart is stricter: an absent or
    /// unconfirmable source host refuses rather than acquiring that default.
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
    /// Checkout membership wins over launch provenance; Clone uses this repo
    /// for a fresh destination, while Replace with keeps the source folder.
    pub(super) repo: Option<GithubRepo>,
    pub(super) title: String,
    /// The row's raw launch command: the seed for custom-command mode.
    pub(super) invocation: String,
    /// The source's agent launch choices, for an agent launch.
    ///
    /// Absent for a command or legacy launch. Clone must preserve that
    /// absence rather than reverse-engineering a harness from a command.
    pub(super) launch: Option<LaunchSelection>,
    /// The source's command launch, whole, for a command launch: its
    /// assertion, agent type and resume command seed the command fields.
    /// Absent for a legacy session, whose command alone (in `invocation`)
    /// is filled in (SPEC.md, the launch-kinds upgrade).
    pub(super) command: Option<CommandLaunch>,
    /// The operation this prefill opens. Replace seeds the source folder
    /// instead of Clone's fresh checkout and holds the source host. Launch
    /// choices, generations and remembered permissions use the same seeding
    /// rules; [`IntentBinding::replace_source`] carries the endpoint projection
    /// through asynchronous submission.
    pub(super) mode: LauncherMode,
}

/// Build the prefill a clone click seeds the create form with, from the
/// clicked row's own `Session`.
///
/// Kept as a pure mapping — apart from `CreateSessionForm` and from whatever
/// mints `generation` (`list::view::ListView`, once per clone click) — so
/// what a clone carries is checkable without mounting a component.
///
/// Always starts in [`LauncherMode::Create`]: a clone's source seeds the
/// draft but does not change the operation. The caller marks Replace with
/// or Restart with afterward, since only it knows which action opened it.
pub(super) fn prefill_from(session: &Session, generation: u64) -> CreatePrefill {
    CreatePrefill {
        generation,
        host: session.host,
        // The form's binding compares raw registry identities. Map each layer
        // separately so absent evidence never becomes an explicit null claim.
        host_identity: session
            .host_identity
            .as_ref()
            .map(|identity| identity.as_ref().map(|value| value.raw().to_owned())),
        cwd: session.cwd.raw().to_owned(),
        repo: session
            .working_copy
            .as_ref()
            .map(|checkout| checkout.repo.clone())
            .or_else(|| session.github_repo.clone()),
        // Clone's local draft and untouched-submit seed both start from the
        // original bytes; editable escaping happens when that seed is applied.
        title: session.title.raw().to_owned(),
        invocation: session.invocation.raw().to_owned(),
        launch: session.agent_selection().cloned(),
        command: match &session.launch {
            Some(crate::SessionLaunch::Command(command)) => Some(command.clone()),
            Some(crate::SessionLaunch::Agent { .. })
            | Some(crate::SessionLaunch::Legacy { .. })
            | None => None,
        },
        mode: LauncherMode::Create,
    }
}

/// An agent type's value in the command form's agent-type select: its
/// protocol spelling, so the option values are stable whatever the labels.
fn harness_value(harness: LaunchHarness) -> String {
    match serde_json::to_value(harness) {
        Ok(serde_json::Value::String(name)) => name,
        _ => String::new(),
    }
}

/// The id of the launcher's "replace with" warning, which the replace button
/// references as its description while the warning has text.
const REPLACE_WARNING_ID: &str = "launch-composer-replace-warning";

/// The displayed stop-first consequence is part of restart consent, including
/// keyboard submits and YOLO answers drawn while that consequence is visible.
const RESTART_WARNING_ID: &str = "launch-composer-restart-warning";

/// What a "replace with" launcher warns about its source, and the precondition
/// its replace sends, from the source state one render shows.
///
/// SPEC.md "Replace with": the launcher's button is the confirmation, so it
/// authorizes only what the launcher said. A source with anything alive gets
/// Replace's warning and the level that warning covers
/// (`status::delete_guard`); a source showing nothing alive gets no warning
/// and `NothingAlive`, so a source restarted behind the page's back is kept
/// rather than killed unannounced. No state at all is treated as nothing
/// shown: no warning, the strictest level.
fn replace_with_warning(
    state: Option<&(SessionStatus, usize)>,
) -> (Option<String>, crate::DeleteGuard) {
    match state {
        Some((status, tabs)) if !crate::status::shows_nothing_alive(status, *tabs) => (
            Some(crate::status::replace_with_consequence(status, *tabs)),
            crate::status::delete_guard(status, *tabs),
        ),
        _ => (None, crate::DeleteGuard::NothingAlive),
    }
}

/// Mark a clone's prefill as a "replace with" of `session`, the row it was
/// built from: the source id that routes the submit to the replace endpoint,
/// plus the status snapshot the launcher falls back to while the live listing
/// lacks the row (see [`LauncherMode::source_opened`]).
/// The source reference remains raw operation data, even when its suffix needs
/// escaping for presentation; the submit must address the original session.
pub(super) fn mark_replace_with(prefill: &mut CreatePrefill, session: &Session) {
    prefill.mode = LauncherMode::Replace {
        source: session.id.raw().to_owned(),
        opened: (session.status.clone(), session.tabs.len()),
    };
}

/// Seed a restart from stored bytes rather than applying new-launch defaults.
/// The source remains a plain-folder destination even for a managed checkout.
/// A source without an editable launch refuses; callers must not open the
/// still-create prefill as a fallback for an existing-session operation.
pub(super) fn mark_restart_with(prefill: &mut CreatePrefill, session: &Session) -> bool {
    if let Some(baseline) = api::RestartEdit::baseline(session) {
        let conversation_required = matches!(baseline, api::RestartEdit::Agent(_))
            && session.restart_offer != crate::RestartOffer::Resume;
        if let api::RestartEdit::Command(command) = &baseline {
            prefill.invocation = command.command.clone();
        }
        prefill.launch = match &baseline {
            api::RestartEdit::Agent(selection) => Some(selection.clone()),
            _ => None,
        };
        prefill.mode = LauncherMode::Restart {
            source: session.id.raw().to_owned(),
            baseline,
            opened_launch: session.launch.clone().map(Box::new),
            conversation_required,
        };
        true
    } else {
        false
    }
}

// ---------------------------------------------------------------------
// A clone's host choice, reconciled against the registry
// ---------------------------------------------------------------------

/// What has become of the current clone generation's host binding.
///
/// Host identity is installation-specific. Keeping this state about the host
/// alone prevents a delayed registry answer or later retarget from owning the
/// cloned launch, which is valid on every host.
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
/// mounting a component or an effect. The cloned launch is seeded once by the
/// reseed effect and is deliberately absent here.
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

/// What a submit should SEND for one of the form's peer-relayed text fields.
///
/// A field nobody typed in sends the RAW seed — what it displays is the
/// escaped rendering of exactly that, so sending the rendering would rewrite a
/// value the user never touched. A field they DID type in sends what they
/// typed, whatever it looks like: someone who replaces a live right-to-left
/// override with its visible spelling means the visible spelling, and no
/// comparison against the seed can tell that apart from not having edited at
/// all (which is why this takes an `edited` flag rather than comparing).
/// The draft's seed transitions preserve this ownership alongside the display.
pub(crate) fn submitted_field(text: &str, edited: bool, seed: Option<&str>) -> String {
    match seed {
        Some(seed) if !edited => seed.to_string(),
        _ => text.to_string(),
    }
}

/// Whether the title a Clone or Replace with carried in stops counting as a
/// name once the destination is a fresh GitHub checkout.
///
/// Clone and Replace with copy the source's title into the name field, and
/// for an existing-folder launch that copy is what the new session should be
/// called. A fresh checkout is different: its directory is named after the
/// title, and SPEC.md (Managed checkouts) refuses an explicit name that
/// is taken rather than suffixing it. The copied title usually names the
/// source's own checkout, so treating it as typed made every Clone or
/// Replace with into `gh:` of the same repository fail with a directory
/// conflict. A title the person never edited is not a choice they made, so
/// with a fresh checkout as the destination the session is unnamed instead
/// and gets the next free `repo-N`, as a blank New form would.
///
/// A seed (`Some`) is what marks the field as clone-carried; an ordinary New
/// form has none, and anything the person types sets `edited`, which makes
/// the field theirs again regardless of destination.
fn copied_title_ignored(edited: bool, seed: Option<&str>, fresh_checkout: bool) -> bool {
    fresh_checkout && !edited && seed.is_some()
}

/// The one source currently entitled to supply a checkout-name policy.
/// Keeping Fixed here preserves its template kind for Save as template, even
/// though it never contributes an automatically numbered candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CheckoutNameSource {
    Clone(GithubRepo),
    Template {
        base: String,
        kind: farhelm_proto::launcher::TemplateNameKind,
    },
}

/// A default or template name scoped to the current host and repository.
/// A hand edit replaces this source; a template replaces any Clone series.
/// Destination changes restart occupancy search without losing the source name
/// needed when switching back to an ordinary folder or saving a template.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CheckoutNaming {
    source: Option<CheckoutNameSource>,
    target: Option<(Option<CreateTarget>, Option<GithubRepo>)>,
    sequence: CheckoutNameSequence,
}

impl CheckoutNaming {
    /// Occupancy belongs to one host installation and repository. Returning
    /// to a prior target starts its search over, rather than caching names.
    fn retarget(&mut self, host: Option<CreateTarget>, repo: Option<GithubRepo>) {
        let target = Some((host, repo));
        if self.target != target {
            self.target = target;
            self.sequence = CheckoutNameSequence::default();
        }
    }

    /// Preserve a template's base and policy across snapshots and destination
    /// changes. Saving the displayed suffix would pin the next launch to a name
    /// that this series has already occupied.
    fn template_name(&self) -> Option<(&str, farhelm_proto::launcher::TemplateNameKind)> {
        match &self.source {
            Some(CheckoutNameSource::Template { base, kind }) => Some((base, *kind)),
            _ => None,
        }
    }

    /// Only a source default or template Base may advance on occupied names.
    /// Fixed names and manually typed names remain exact; ordinary folders use
    /// the raw title and permit duplicate session names as before.
    fn default_title(
        &self,
        edited: bool,
        seed: Option<&str>,
        repo: Option<&GithubRepo>,
    ) -> Option<String> {
        match &self.source {
            Some(CheckoutNameSource::Clone(source_repo))
                if !edited && Some(source_repo) == repo =>
            {
                let seed = seed.filter(|seed| !seed.is_empty())?;
                Some(self.sequence.name(&format!("{seed}-clone")))
            }
            Some(CheckoutNameSource::Template {
                base,
                kind: farhelm_proto::launcher::TemplateNameKind::Base,
            }) if repo.is_some() => Some(self.sequence.name(base)),
            _ => None,
        }
    }

    /// Try the next suffix only after the current authoritative preview says
    /// occupied. An unresolved create keeps its name for reconciliation: its
    /// own accepted directory may be the collision. At the cap, retain the
    /// visible conflict for a manual edit.
    fn advance(&mut self, reconciling: bool) -> bool {
        self.sequence.advance(reconciling)
    }
}

/// Hover text for one launcher search result.
///
/// A result's visible line is its kind and value ("Model: …"); the tooltip
/// says what choosing it does to the draft, which the line leaves to the
/// reader. A recent setup instead carries its complete description, because
/// its two-line rendering may truncate the saved values. Peer-supplied values
/// (names, paths, model ids) go through `display_peer`, as in the visible line.
fn search_result_tooltip(
    result: &crate::launch_composer::ComposerSearchResult,
    selected_host_label: &str,
    harness: Option<LaunchHarness>,
) -> String {
    use crate::launch_composer::{ComposerPermission, ComposerSearchResult};
    match result {
        ComposerSearchResult::Recent(entry) => format!(
            "{} · {} · {}",
            display_peer(&crate::launch_composer::recent_destination_label(entry)),
            selected_host_label,
            display_peer(&crate::launch_composer::selection_summary(&entry.selection)),
        ),
        ComposerSearchResult::Name(name) => {
            format!("Use \"{}\" as the session's name", display_peer(name))
        }
        ComposerSearchResult::Host(_) => "Run the session on this host".to_string(),
        ComposerSearchResult::UsePath(_) | ComposerSearchResult::Folder(_) => {
            "Use this folder: the session will start here".to_string()
        }
        ComposerSearchResult::BrowsePath(_) => "List the folders under this path".to_string(),
        ComposerSearchResult::Harness(_) => "Use this agent for the session".to_string(),
        ComposerSearchResult::Template(_) => {
            "Apply this template's settings to the draft".to_string()
        }
        ComposerSearchResult::Github(_) => {
            "Start in a managed checkout of this repository".to_string()
        }
        ComposerSearchResult::Model { .. } => "Use this model for the session".to_string(),
        ComposerSearchResult::Effort(Some(_)) => {
            "Use this reasoning effort for the session".to_string()
        }
        ComposerSearchResult::Effort(None) => {
            "Use the agent's default reasoning effort".to_string()
        }
        ComposerSearchResult::Permissions(ComposerPermission::Guarded) => {
            crate::launch_composer::guarded_permission_help(harness)
        }
        ComposerSearchResult::Permissions(ComposerPermission::OwnSetting) => {
            "Use the agent's own setting, which Farhelm cannot force or classify".to_string()
        }
        ComposerSearchResult::Permissions(ComposerPermission::Custom) => {
            crate::launch_composer::permission_tooltip(LaunchPermission::Custom).into()
        }
        ComposerSearchResult::Permissions(ComposerPermission::Yolo) => {
            crate::launch_composer::permission_tooltip(LaunchPermission::Yolo).to_string()
        }
        ComposerSearchResult::Permissions(ComposerPermission::Approve) => {
            crate::launch_composer::permission_tooltip(LaunchPermission::Approve).to_string()
        }
        ComposerSearchResult::Permissions(ComposerPermission::SmartApprove) => {
            crate::launch_composer::permission_tooltip(LaunchPermission::SmartApprove).to_string()
        }
        ComposerSearchResult::Permissions(ComposerPermission::Chat) => {
            crate::launch_composer::permission_tooltip(LaunchPermission::Chat).to_string()
        }
    }
}

/// One effective title for display, preview, retry bindings and submission.
///
/// Clone into its source repository uses the supplied untouched `-clone`
/// default. Other fresh-checkout destinations ignore a copied title and
/// allocate an unnamed `repo-N`; folders and explicit edits keep their raw
/// spelling. Deciding on read preserves the source title when a folder is
/// chosen again. All launch-time rechecks must use this same policy: a name
/// that differs from the preview is refused as stale. An empty string already
/// requests an unnamed checkout (`checkout_basename` treats it as absent), so
/// this policy needs no helm or supervisor change.
fn submitted_title(
    text: &str,
    edited: bool,
    seed: Option<&str>,
    fresh_checkout: bool,
    clone_default: Option<&str>,
) -> String {
    if let Some(default) = clone_default {
        default.to_string()
    } else if copied_title_ignored(edited, seed, fresh_checkout) {
        String::new()
    } else {
        submitted_field(text, edited, seed)
    }
}

/// Read the resolved name that display, preview and submission must share.
/// Clone defaults and template bases use the same bounded suffix series.
/// `launcher_snapshot` separately preserves a template's base and kind so
/// saving a setup does not capture a suffix belonging to this one allocation.
fn effective_title(
    text: &str,
    edited: bool,
    seed: Option<&str>,
    destination: &DestinationDraft,
    naming: &CheckoutNaming,
) -> String {
    let default = naming.default_title(edited, seed, destination.repo());
    submitted_title(
        text,
        edited,
        seed,
        destination.repo().is_some(),
        default.as_deref(),
    )
}

/// The session-launch dialog, including the structured composer and the
/// explicit legacy fallback.
///
/// A new dialog opens on the structured surface with no harness selected.
/// Arbitrary commands remain available through the legacy surface, but
/// cannot contribute hidden values to a structured request.
///
/// The page-owned operation token keeps new launches and host mutations out
/// while a submit is in flight; session navigation remains available. This form owns its guard:
/// cancellation releases the token, but cannot recover an uncertain create
/// reply or run the success callback after the form is gone. Ordinary close
/// controls therefore refuse to unmount a submitting form.
///
/// `on_created` fires only on a successful POST, with the newly created
/// `Session` from the response body and the launch this form submitted
/// ([`CreatedSession`]); `ListView` uses that to close the
/// form and decide whether its accepted answer still permits opening the new
/// session in the adjacent pane. Page input since that answer leaves the user
/// where they moved; the sidebar itself stays
/// mounted throughout. On failure the form stays mounted with its values
/// untouched and the refusal rendered beside Launch — the draft remains local
/// to this component rather than being reset or lifted into
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
/// ## The command path
///
/// Ordinary New starts on the agent tab with no agent type chosen; the
/// command tab holds a command launch's own draft. Clone and Replace instead
/// seed their source's agent choices or exact command. Changing the host leaves the draft
/// intact. The command path is what runs anything the composer does not
/// describe, and the e2e harness's own creates go through it.
///
/// ## Clone prefill
///
/// A "clone" click (`ListView`'s `on_clone`) opens this same form with
/// `prefill` set instead of building a second, immediate-create path — see
/// `CreatePrefill`'s own doc for what it carries and why. A "replace with"
/// click (`ListView`'s `on_replace_with`) takes the identical path with the
/// identical prefill shape, with `CreatePrefill::mode` selecting Replace.
/// Both modes share launch seeding and host reconciliation; Replace seeds
/// the source folder and holds the host, then projects its source into
/// `IntentBinding::replace_source` for submission. The reseed effect replaces
/// the whole agent draft for each source generation. The launch is independent of
/// host reconciliation, so a delayed or unconfirmable host cannot suppress
/// it, and a later host answer cannot overwrite a person's edits.
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
/// falls through to its ordinary default, while the launch is still seeded
/// from the source; `clone_host_note` (below) tells the user why the
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
/// idempotency only; it does not clear the launch draft.
///
/// ## Restart's held source
///
/// Restart uses those same editors, search, templates and confirmation, with
/// source placement and launch kind held. Stored permission omissions remain
/// own-setting omissions, rather than acquiring create defaults. Reset restores
/// that stored baseline and clears the transient conversation. Folder advice
/// checks only the confirmed source installation; neither history updates nor
/// a withdrawn host binding may move the held folder to another destination.
/// Submission uses the existing restart endpoint without a retry key. Its
/// confirmation token is local draft identity only, and every actual POST
/// result reaches the view's reattachment hooks even when this form stays open.
#[component]
pub(super) fn CreateSessionForm(
    hosts: Vec<HostOption>,
    /// The selected session's host and reported installation identity,
    /// carried by `ListView`'s `open_destination` snapshot. This supplies
    /// SPEC.md's first create-default clause independently of sidebar filtering.
    open_host: Option<OpenHost>,
    /// The selected session's folder, from the same snapshot as `open_host`.
    /// A host change back to that session's installation seeds it again,
    /// ahead of the host's remembered folder (SPEC.md, "Working directory").
    #[props(default)]
    open_cwd: Option<String>,
    /// Whether the hosts read has EVER succeeded. Distinguishes "there are
    /// no hosts" (impossible for a live helm, which always has its local
    /// row) from "nothing has come back yet", which is what a submit has to
    /// be refused for.
    hosts_loaded: bool,
    /// A completed initial host-read failure must reveal quick launch rather
    /// than treating a failed read as indefinitely pending. Retained snapshots
    /// still follow ordinary New's availability checks.
    host_read_error: Option<String>,
    /// The user's explicit host choice, if they have made one. `None` means
    /// "no choice yet", not "no host" — the effective target is
    /// [`effective_create_host`]'s answer, recomputed per render against the
    /// hosts that exist right now.
    ///
    /// `ListView`'s signal rather than this form's so the same effective-host
    /// derivation also binds the idempotency key and connection claim.
    chosen_host: Signal<Option<HostId>>,
    /// The installation currently selected for the create, for host safety
    /// checks.
    create_target: Signal<Option<CreateTarget>>,
    /// The page's live-operation token excludes host changes and another launch.
    /// A hidden request keeps it through an owed Return admission, independently
    /// of the acted-on session's earlier reply/claim settlement (see `ops`).
    mut ops: OpLock,
    /// Directory copied from the currently selected session for an ordinary
    /// New action. `None` preserves the portable target-home default.
    initial_cwd: Option<String>,
    /// A "clone" click's seed, or `None` for the ordinary blank-form open.
    /// `ListView` owns the signal this reads and bumps `generation` on
    /// every clone (see `CreatePrefill`); this component's own reseed
    /// effect is what turns a new
    /// generation into field values.
    prefill: Option<CreatePrefill>,
    /// One accepted switcher action for an ordinary New draft. Applied once
    /// after mount seeding (templates also wait for asynchronous inputs),
    /// through the same search-accept path as a keyboard or mouse choice. This is not a clone
    /// prefill: remembered permissions and New's destination still seed normally.
    initial_action: Option<crate::launch_composer::ComposerSearchResult>,
    /// Quick-launch definitions captured together before this form mounted.
    /// Replay one per effect pass so host and catalog reconciliation observes
    /// each preceding edit before the next template is validated against it.
    #[props(default)]
    initial_templates: Vec<farhelm_proto::launcher::LaunchTemplate>,
    /// Visible-launcher shortcut presses arrive as separate ordered batches.
    /// The parent resolves names once; this form alone decides whether each
    /// captured definition fits and applies it through ordinary search edits.
    shortcut_requests: Signal<std::collections::VecDeque<ShortcutApplyRequest>>,
    /// Explain why the initiating quick-launch action opened an incomplete draft.
    /// The notice belongs to this opening, never to a later Clone/Replace seed.
    #[props(default)]
    partial_template_banner: Option<crate::launch_sequence::PartialTemplateBanner>,
    /// Quick launch submits this ordinary draft once, after its template and
    /// prerequisites settle. Failures reveal the same mounted form for retry.
    auto_launch: Signal<bool>,
    /// Shared with the parent's read admission and ordinary opening actions.
    /// A superseding opening revokes even an already queued DOM submission.
    opening_generation: Signal<u64>,
    /// The list's mounted draft owner, which a modal epoch alone cannot identify.
    mounted_opening: u64,
    /// Captured before an accepted quick launch starts its asynchronous preparation.
    /// Its automatic form submit continues that answer instead of resetting it.
    #[props(default)]
    initial_quick_baseline: Option<u64>,
    /// A denied return settles its request, then retires only its mounted
    /// draft owner. This is not Cancel and must not close a newer opening.
    on_retired: EventHandler<u64>,
    /// Every create reply refreshes the persistent list before any Return wait.
    /// A replacement may have removed its source even when its reply is refused.
    on_settled: EventHandler<()>,
    /// Retire only the list's unused startup pick before this client's new
    /// session can appear in a feed. Replace keeps its source-selection rule.
    on_create_dispatch: EventHandler<()>,
    /// A failed quick-launch preparation is visible without losing New's defaults.
    #[props(default)]
    initial_error: Option<String>,
    /// For "replace with", the source session's status and terminal tab
    /// count as the page currently knows them: its row in the live listing,
    /// else the last state a listing showed for it, else
    /// [`LauncherMode::source_opened`].
    /// `None` for a create or a plain clone.
    ///
    /// SPEC.md "Replace with": the launcher's replace button is the
    /// confirmation, so while this shows anything alive the launcher draws
    /// Replace's warning, and the replace it sends carries the precondition
    /// matching that same value (`status::delete_guard`). Both are derived
    /// from this one prop in the same render, so the request never authorizes
    /// more than the text on screen said.
    replace_source_state: Option<(SessionStatus, usize)>,
    /// Restart reports through the persistent list even after the originating
    /// keyed view disappears; its source slot follows current listing evidence.
    #[props(default)]
    restart_bridge: Option<crate::list::RestartBridge>,
    /// Successful Restart retires only the submitted mounted opening's seeds.
    /// Shared terminal and claim settlement already happened through the bridge.
    #[props(default)]
    on_restarted: EventHandler<u64>,
    /// Discard this draft without creating a session.
    on_cancel: EventHandler<()>,
    on_created: EventHandler<CreatedSession>,
    /// A template save closes the launcher and hands its stored snapshot to the editor.
    on_template_saved: EventHandler<farhelm_proto::launcher::LaunchTemplate>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let launch_activity = use_context::<LaunchActivity>();
    let mut session_operations = use_context::<crate::session_ops::SessionOps>();
    let shared_replace_progress = use_context::<crate::replace::ReplaceProgress>();
    // Catalog reads retain complete targets while the existing draft owner
    // controls edits and launch admission. History joins only reusable intent.
    let (environment_catalog, _refresh_environment_places) =
        crate::environment_ui::use_environment_places();
    let environment_places = environment_catalog
        .read()
        .as_ref()
        .and_then(|answer| answer.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    // Build projections include cached place refusals. A completed probe changes
    // this successful observation without a fleet notice; reread the projection,
    // while equal cached answers leave it alone and never force another probe.
    let observed_places = use_memo(move || {
        environment_catalog
            .read()
            .as_ref()
            .and_then(|answer| answer.as_ref().ok())
            .cloned()
    });
    let build_status_base = base.clone();
    let mut build_status = use_resource(move || {
        let _places = observed_places();
        let base = build_status_base.clone();
        async move { api::fetch_recipe_places(&base).await }
    });
    let environment_history_base = base.clone();
    let mut environment_history = use_resource(move || {
        let base = environment_history_base.clone();
        async move { api::fetch_environment_history(&base).await }
    });
    let sandbox_base = base.clone();
    let mut sandbox_catalog = use_resource(move || {
        let base = sandbox_base.clone();
        async move {
            let recipes = api::fetch_recipes(&base).await?;
            let settings = api::fetch_tensorlake_settings(&base).await?;
            Ok::<_, String>((recipes, settings))
        }
    });
    use_feed_reader(move || {
        build_status.restart();
        environment_history.restart();
        sandbox_catalog.restart();
    });
    // A saved container place needs its first cached answer before replay or
    // automatic submit can classify it. Read errors settle this wait and keep
    // the ordinary refusal path in charge. This read never forces a probe.
    let place_catalog_pending = use_callback(move |target: Option<farhelm_proto::LaunchTarget>| {
        target.is_some_and(|target| target.place().container_host().is_some())
            && environment_catalog.read().is_none()
    });
    let sandbox_answer = sandbox_catalog.read().as_ref().cloned();
    let recipe_names = sandbox_answer
        .as_ref()
        .and_then(|answer| answer.as_ref().ok())
        .map(|(recipes, _)| {
            recipes
                .iter()
                .map(|recipe| recipe.name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // The helm-wide preference row (`PreferencesGate`'s seed): read here
    // only to seed a fresh no-prefill dialog's permissions segment and to
    // answer "reset choices" — see `initial_structured_permissions`. This
    // form never WRITES through this signal; the helm sets
    // `remembered_permissions` as a side effect of a successful launch, and
    // `list::view`'s `on_created` handler mirrors this client's own result
    // into it afterward.
    let preferences = use_context::<SharedPreferences>();
    // A pending quick-launch sequence owns Enter: template-shortcuts.js finishes it from
    // the window, after this form's handlers. The search box must not also act on that key.
    let launch_sequence =
        try_use_context::<crate::launch_sequence::LaunchSequence>().map(|sequence| sequence.0);
    let launch_catalog_base = base.clone();
    let mut launch_catalog = use_resource(move || {
        let base = launch_catalog_base.clone();
        async move { api::fetch_launch_catalog(&base).await }
    });
    // Keep failure distinct from an empty catalog: `tl:` offers a retry,
    // and an initial named template must not be reported as deleted when
    // its catalog could not be read. Unrelated launch choices stay usable.
    let templates_base = base.clone();
    // Re-read whenever the Templates dialog closes (see `TemplatesRevision`);
    // absent outside the list view, where nothing can change them.
    let templates_revision = try_use_context::<super::templates::TemplatesRevision>();
    let mut launch_templates = use_resource(move || {
        let base = templates_base.clone();
        let _revision = templates_revision.map(|revision| (revision.0)());
        async move { api::fetch_templates(&base).await }
    });
    // Why the last template accepted from search was not applied, shown
    // under the search box until the next accepted result.
    let mut template_error = use_signal(|| None::<String>);
    let mut repository_open = use_signal(|| false);
    let mut repository_index = use_signal(|| 0_usize);
    use_effect(move || {
        if repository_open() {
            scroll_repository_result(repository_index());
        }
    });
    let mut github_attempt = use_signal(|| None::<(IntentBinding, GithubAttempt)>);
    // A Replace-with deletes its source, whose notice the session list shows.
    let delete_notice = use_context::<super::DeleteNotice>();
    let mut preview_generation = use_signal(|| 0_u64);
    let mut preview_revision = use_signal(|| 0_u64);
    let mut observed_checkout_revision = use_signal(|| 0_i64);
    let mut live_preview_authority = use_signal(|| None::<PreviewAuthority>);
    // Replies are observations, not destination edits. A completion wakes only
    // preview consumers and cannot overwrite semantic repository ownership.
    let mut preview_state = use_signal(|| PreviewState::Pending);
    // This opening's ordinary retry pair is shared with the edit owner.
    // Submission still mints/reconciles it through this same handle while
    // holding the operation claim; edit-induced retirement belongs to draft.
    let mut intent_key = use_signal(|| None::<(String, IntentBinding)>);
    // Seeded from the helm's remembered permissions mode ONLY when this
    // mount has no prefill at all (SPEC.md's launch-composer carve-out): a
    // clone/replace prefill's own reseed effect below (`prefill.launch`)
    // unconditionally overwrites this seed the moment it runs, so a
    // prefilled mount briefly starting at `None` here is never visible —
    // see `initial_structured_permissions`'s own doc for why the precedence
    // is a call-site guard rather than logic inside that function.
    // Placement mounts once from the inherited raw cwd, or the editable "~"
    // default. Expansion belongs to the target supervisor, not this browser.
    let draft = use_launch_draft(
        if prefill.is_none() {
            initial_structured_permissions(&preferences.0.peek())
        } else {
            None
        },
        initial_cwd.clone(),
        ops,
        intent_key,
        chosen_host,
    );
    // Whether `structured_permissions`'s current value is a real choice
    // (an explicit click, a restored prefill, or an applied recent/search
    // row) rather than the passive memory seed above or "reset choices"
    // re-applying it. `recent_filter` below reads this, not
    // `structured_permissions` directly: `ComposerFilter`'s whole contract
    // is that an absent dimension excludes nothing (see that type's own
    // doc), and a memory-seeded permissions value the person never asked
    // for must not start silently hiding an otherwise-relevant recent
    // whose OWN permissions choice happens to differ — the exact failure
    // this flag exists to prevent turned up as a genuine recent-slot
    // regression while validating this feature (a fixture recent with
    // `permissions: null` vanished once an earlier launch in the same
    // browser session had remembered `"yolo"`). Every other seeded field
    // (harness/model/effort) has no passive-seed case to distinguish from
    // — SPEC.md keeps them unpreselected on a fresh open — so only
    // permissions needs a separate explicitness bit in the draft.
    // These are read-only subscriptions, never independently mutable fields.
    // Handler-time authority reads the live owner rather than memo.peek(),
    // whose cached value can lag an earlier edit in the same event turn.
    let DraftFields {
        recipe,
        environment_target,
        cwd,
        cwd_edited,
        title,
        title_raw_seed,
        title_edited,
        destination_draft,
        clone_checkout_naming,
        launch_tab,
        invocation,
        invocation_edited,
        command_yolo,
        command_agent,
        command_resume_on,
        command_resume,
        command_resume_edited,
        prompt,
        conversation,
        structured_harness,
        structured_model,
        structured_model_raw_seed,
        structured_model_edited,
        structured_effort,
        structured_permissions,
        structured_permissions_is_explicit,
        structured_permission_flags,
        structured_wrapper,
        wrapper_is_explicit,
        container,
    } = draft.fields();
    // Compatibility clears are intentional, but defaults must never make an
    // earlier explicit choice vanish without telling the person what changed.
    let composer_reset_reason = use_signal(|| None::<String>);
    let mut permission_escalation = use_signal(|| None::<PermissionEscalation>);
    let permission_escalation_generation = use_signal(|| 0_u64);
    use_effect(move || {
        let opening = opening_generation();
        if permission_escalation
            .peek()
            .as_ref()
            .is_some_and(|notice| notice.opening_generation != opening)
        {
            permission_escalation.set(None);
        }
    });
    let mut composer_search = use_signal(String::new);
    let mut composer_search_open = use_signal(|| false);
    let mut composer_search_index = use_signal(|| 0_usize);
    // The input is a view of this draft while open, not the source of a
    // selection: blur and Escape can therefore discard unaccepted text.
    let mut model_draft = use_signal(String::new);
    let mut model_open = use_signal(|| false);
    // Only Arrow navigation makes a row active. Typing is a custom-id draft
    // until the person deliberately navigates to one of its suggestions.
    let mut model_active = use_signal(|| None::<usize>);
    let mut model_show_all = use_signal(|| false);
    let mut model_draft_error = use_signal(|| None::<String>);
    // Opening never selects: Enter still submits a typed path until an arrow
    // deliberately gives an option keyboard authority. Blur discards that authority.
    let mut folder_open = use_signal(|| false);
    let mut folder_active = use_signal(|| None::<usize>);
    use_effect(move || {
        if folder_open()
            && let Some(index) = folder_active()
        {
            scroll_folder_result(index);
        }
    });
    // Keep the host's readable label with the same immutable choice snapshot;
    // a later host pick must not relabel the identity this panel will save.
    let mut template_panel = use_signal(|| None::<(super::save_template::Candidate, String)>);
    let template_saving = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    // Preparation describes the opening, not a refusal from a bound host.
    // First binding and later host reconciliation must not erase its explanation.
    let mut preparation_error = use_signal(move || initial_error);
    let quick_origin = use_hook(|| *auto_launch.peek());
    let mut quick_launch_hidden = use_signal(move || *auto_launch.peek());
    let mut auto_launch_pending = use_signal(move || *auto_launch.peek());
    let quick_press_baseline = use_hook(|| initial_quick_baseline);
    // Nested Option distinguishes an automatic submit with unknown evidence
    // from a later explicit answer, which snapshots the current mirrored count.
    let mut automatic_submit_baseline = use_signal(|| None::<Option<u64>>);
    use_effect(move || {
        if !auto_launch() && quick_launch_hidden() {
            auto_launch_pending.set(false);
            quick_launch_hidden.set(false);
        }
    });
    // An untouched dialog is allowed to be incomplete. Once attempted, its
    // prerequisite reason follows the current draft and disappears when ready;
    // request failures retain the helm's own words until the next attempt.
    let mut launch_attempted = use_signal(|| LaunchAttempt::None);
    // Only this launch API call owns the snapshot. Launcher preparation and
    // another page operation may be busy too, but neither is this request's wait.
    let mut pending_launch = use_signal(|| None::<PendingLaunch>);
    let mut sandbox_question = use_signal(|| None::<LaunchDraft>);
    let mut sandbox_consent = use_signal(|| None::<LaunchDraft>);
    let mut sandbox_run = use_signal(|| None::<(HostId, String)>);
    let progress_base = base.clone();
    use_future(move || {
        let base = progress_base.clone();
        async move {
            loop {
                crate::reader::sleep_ms(1_000).await;
                let run = sandbox_run.peek().clone();
                if let Some((host, run)) = run
                    && let Ok(progress) = api::fetch_provisioning(&base, host).await
                    && progress.run_id.as_deref() == Some(run.as_str())
                    && sandbox_run.peek().as_ref() == Some(&(host, run))
                    && let Some(step) = progress.steps.iter().find(|step| step.status == "running")
                    && let Some(pending) = pending_launch.write().as_mut()
                {
                    let name = step
                        .name
                        .as_ref()
                        .map(|name| name.raw().to_string())
                        .unwrap_or_else(|| step.step.clone());
                    pending.observe_stage(
                        name,
                        step.explanation.as_ref().map(|text| text.raw().to_string()),
                    );
                }
            }
        }
    });
    // Set only around this form's explicit Resume and reconnect wait. Other
    // viewers' shared Resuming phase must not make a target launchable here.
    // Form unmount cancels the future and retires this signal with its owner.
    let mut resuming_host = use_signal(|| None::<HostResumePreparation>);
    let mut resume_display_handoff = use_signal(|| None::<ResumeDisplayHandoff>);
    // Submission changes presentation without changing quick-launch provenance:
    // that separate flag decides whether success should open session naming.
    // Preparation, billing and intent-key minting retain the ordinary modal.
    let submitted_wait = use_memo(move || pending_launch().is_some() || resuming_host().is_some());
    let composer_hidden = use_memo(move || quick_launch_hidden() || submitted_wait());
    // Retain both boundaries: a resumed quick launch may return to modal
    // preparation while remaining hidden. Dispatch arms this before rendering
    // too, so a fast reply cannot skip the isolation restoration it owes.
    let mut previous_presentation = use_signal(move || (*quick_launch_hidden.peek(), false));
    // A YOLO launch the helm refused because its host asks before YOLO launches, and the
    // user's confirmation of one (see `yolo_confirm`). Both are tied to the intent key the
    // refused request carried, not to "the next submit": the key is bound to the whole
    // draft (host, folder, launch), so any edit that changes what would launch retires the
    // key and with it the confirmation, and a confirmation for host A can never ride along
    // with a launch on host B. Holding the confirmed key rather than a one-shot flag also
    // keeps the override on every retry of the SAME intent: a confirmed create whose reply
    // was lost must replay under its key with the override, or the helm would refuse the
    // replay and a second confirmation would mint a new key and a second session.
    //
    // The question itself is a `ConfirmSlot` (`LauncherYoloQuestion`): a
    // YOLO answer records consent only by taking the opening it was drawn for,
    // then submits the form with the Replace precondition its own render
    // showed, so an answer queued behind Cancel records nothing and submits
    // nothing (`answer_launcher_yolo`).
    //
    // "Start, and don't ask again on this host" adds a request to mark the
    // host safe first, held the same way: the token of the opening it
    // answered (and so its refused key), set by that button's click just
    // before it submits the form, and taken by that one submit, so it can
    // never ride along with a later Launch or "start anyway". `yolo_error`
    // is that first step's failure; the question comes back up with it.
    let mut yolo_refusal = use_confirm_slot::<LauncherYoloQuestion, ()>();
    let mut yolo_confirmed_key = use_signal(|| None::<String>);
    let mut yolo_stop_asking = use_signal(|| None::<ConfirmToken<LauncherYoloQuestion>>);
    let mut yolo_error = use_signal(|| None::<String>);
    // A visible guarded answer waits without a claim. Wake continuation keeps
    // its original token and optional Replace source in this same slot, so
    // neither exclusion opens during preview or internal submit reentry.
    let mut guarded_submit_pending =
        use_signal(|| None::<(GuardedSubmitStamp, Option<SubmitGuards>)>);
    // Bumped each time a successful wake parks its press in the slot above;
    // each value gets one bounded wait for the woken connection. The stamp has
    // no identity of its own, so without this a quick retry in the same
    // opening could be timed out by an earlier attempt's wait.
    let mut woken_connection_wait = use_signal(|| 0_u64);
    // The intent key of the last attempt whose outcome stayed unknown (a
    // failure that kept the key for a safe retry). See
    // `LauncherYoloQuestion::uncertain`. It is never cleared: every attempt
    // key is freshly minted at random, so a mark left on a retired key can
    // never match a later refusal.
    let mut uncertain_key = use_signal(|| None::<String>);
    // Open the question for `refused_key`; the slot makes it a new opening,
    // so an answer drawn for an earlier asking about the same refused
    // request cannot take it.
    let mut open_yolo_question = move |refused_key: String, ask: crate::yolo_confirm::YoloAsk| {
        let uncertain = uncertain_key.peek().as_deref() == Some(refused_key.as_str());
        yolo_refusal.open(
            LauncherYoloQuestion {
                refused_key,
                ask,
                uncertain,
            },
            (),
        );
    };
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
    let clone_host_state = use_signal(|| CloneHostState::Unconfirmable);
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
                "The session you cloned predates host tracking, so its host could not be \
                 confirmed and the ordinary host default is used — check the host below",
            ),
            // Worded for the SOURCE session rather than "the session you
            // cloned": a replace-with prefill takes this same path, and its
            // user did not clone anything.
            CloneHostState::Unconfirmable => Some(
                "The source session reports a different installation now, so its host was \
                 not carried over here — check the host below",
            ),
            CloneHostState::Waiting | CloneHostState::Bound | CloneHostState::UserTookOver => None,
        });
    // Taken before the reseed effect below moves the prop into its own
    // `move` closure: the submit handler needs the mode's source and `host`
    // long after that effect has run (one owned copy, consumed by its
    // closure), and the launch button's verb only needs to know whether
    // this is a replace-with at all — a `bool`, derived here rather than
    // a second clone of the whole prefill.
    let prefill_for_submit = prefill.clone();
    let is_replace_with = prefill
        .as_ref()
        .is_some_and(|prefill| prefill.mode.replace_source().is_some());
    let host_fixed = prefill
        .as_ref()
        .is_some_and(|prefill| prefill.mode.holds_host());
    let restart_mode = prefill
        .as_ref()
        .is_some_and(|prefill| prefill.mode.is_restart());
    let restart_baseline = prefill.as_ref().and_then(|prefill| match &prefill.mode {
        LauncherMode::Restart { baseline, .. } => Some(baseline.clone()),
        _ => None,
    });
    let conversation_required = prefill.as_ref().is_some_and(|prefill| {
        matches!(
            prefill.mode,
            LauncherMode::Restart {
                conversation_required: true,
                ..
            }
        )
    });
    let restart_source = restart_bridge.and_then(|bridge| {
        bridge
            .current
            .try_read()
            .ok()
            .and_then(|session| session.clone())
    });
    let restart_stop_first = restart_source
        .as_ref()
        .is_some_and(|source| source.status == SessionStatus::Running);
    let restart_agent_baseline = restart_baseline
        .as_ref()
        .and_then(|baseline| match baseline {
            api::RestartEdit::Agent(selection) => Some(selection.clone()),
            _ => None,
        });
    let held_agent_label = prefill
        .as_ref()
        .and_then(|prefill| {
            prefill
                .launch
                .as_ref()
                .map(|launch| launch.harness)
                .or_else(|| prefill.command.as_ref().and_then(|command| command.agent))
        })
        .map(crate::launch_composer::harness_label)
        .unwrap_or("Unknown agent");
    let creation_origin = (!host_fixed)
        .then(|| {
            prefill
                .as_ref()
                .and_then(|source| source.host)
                .or_else(|| open_host.as_ref().map(|host| host.id))
        })
        .flatten();
    let origin_unresolved =
        unresolved_creation_origin(creation_origin, &hosts, chosen_host(), recipe().as_deref());
    let selected = if origin_unresolved || recipe().is_some() && !host_fixed {
        None
    } else if restart_mode {
        prefill.as_ref().and_then(|prefill| prefill.host)
    } else {
        effective_create_host(&hosts, chosen_host(), open_host.as_ref())
    };
    let restart_host_confirmed = !restart_mode
        || prefill.as_ref().is_some_and(|prefill| {
            prefill.host.is_some_and(|id| {
                matching_host_option(
                    &OpenHost {
                        id,
                        identity: prefill.host_identity.clone(),
                    },
                    &hosts,
                )
                .is_some()
            })
        });
    let selected_folder = open_host
        .as_ref()
        .zip(open_cwd.as_ref())
        .and_then(|(open, cwd)| {
            matching_host_option(open, &hosts).map(|host| {
                (
                    CreateTarget::new(host.id, host.incarnation.clone()),
                    cwd.clone(),
                )
            })
        });
    let destination_now = history_target(&hosts, selected);
    // Store the latest render's registry claim synchronously. Parent-derived
    // create_target has an intentional effect lag, while an old history
    // callback must already refuse a replacement visible in this render.
    let mut live_destination = use_signal(|| None::<CreateTarget>);
    if *live_destination.peek() != destination_now {
        live_destination.set(destination_now.clone());
        resume_display_handoff.set(None);
    }
    // Keep only live target/token facts for the async completion to inspect.
    // Synchronous publication follows live_destination's existing render fence;
    // an effect would leave one more stale observation at the handoff boundary.
    let displayed_resume = resume_display_observation(&hosts, selected);
    let mut live_resume_display = use_signal(|| None::<ResumeDisplayHandoff>);
    if *live_resume_display.peek() != displayed_resume {
        live_resume_display.set(displayed_resume.clone());
    }
    if resume_display_handoff
        .peek()
        .as_ref()
        .is_some_and(|receipt| Some(receipt) != displayed_resume.as_ref())
    {
        resume_display_handoff.set(None);
    }
    let inherited_destination = initial_cwd.as_ref().and(destination_now.clone());
    // An opening that brings a folder (the selected session's on this install,
    // or a Clone/Replace source) owns it; see `folder_default_step`.
    let opened_with_source_folder = initial_cwd.is_some() || prefill.is_some();
    let remembered_destination = use_signal(move || inherited_destination);
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
    let browse_activation_attempts = use_signal(|| 0_u64);
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
    // The last launch refusal that was about the folder; see `FolderRefusal`.
    let mut folder_refusal = use_signal(|| None::<FolderRefusal>);
    // Automatic inspection has its own authority and generation, independent
    // of explicit Browse. Keep an admitted request alive across resource
    // restarts: cancellation of a debounce must not admit overlapping reads.
    let mut folder_check_generation = use_signal(|| 0_u64);
    let mut folder_check_live = use_signal(|| None::<(BrowseAuthority, String)>);
    let mut folder_check_requested = use_signal(|| None::<BrowseAuthority>);
    let mut folder_check_pending = use_signal(|| false);
    let mut folder_check_result = use_signal(|| None::<(BrowseAuthority, FolderProblem, String)>);
    let mut folder_check_completions = use_signal(|| 0_u64);
    let mut proposed_check = selected
        .and_then(|id| {
            hosts
                .iter()
                .find(|host| host.id == id && host.phase.is_none())
        })
        .filter(|_| !destination_draft().is_managed())
        .filter(|_| {
            restart_host_confirmed
                && (!restart_mode
                    || prefill
                        .as_ref()
                        .is_some_and(|prefill| prefill.cwd == draft.snapshot().submitted_cwd()))
        })
        .and_then(|host| {
            let _ = (cwd(), cwd_edited());
            Some((
                BrowseAuthority {
                    target: destination_now.clone()?,
                    connection: connection_claim(&hosts, host.id),
                    cwd: draft.snapshot().submitted_cwd(),
                    generation: *folder_check_generation.peek(),
                },
                host.label(),
            ))
        });
    if *folder_check_live.peek() != proposed_check {
        let generation = folder_check_generation.peek().wrapping_add(1);
        folder_check_generation.set(generation);
        if let Some((authority, _)) = &mut proposed_check {
            authority.generation = generation;
        }
        folder_check_live.set(proposed_check);
    }
    let folder_check_base = base.clone();
    let _folder_check = use_resource(move || {
        let wanted = folder_check_live();
        let pending = folder_check_pending();
        let base = folder_check_base.clone();
        async move {
            let Some((authority, host_label)) = wanted.filter(|_| !pending) else {
                return;
            };
            if folder_check_requested.peek().as_ref() == Some(&authority) {
                return;
            }
            crate::reader::sleep_ms(150).await;
            if folder_check_pending()
                || folder_check_live.peek().as_ref().map(|(claim, _)| claim) != Some(&authority)
            {
                return;
            }
            folder_check_pending.set(true);
            folder_check_requested.set(Some(authority.clone()));
            spawn(async move {
                let response = api::check_directory(
                    &base,
                    authority.target.host,
                    &authority.cwd,
                    authority.connection,
                )
                .await;
                let current = folder_check_live
                    .peek()
                    .as_ref()
                    .map(|(claim, _)| claim.clone());
                if browse_reply_is_current(
                    &current,
                    &authority,
                    live_destination(),
                    live_browse_connection(),
                    &draft.snapshot().submitted_cwd(),
                ) {
                    // Only definitive missing/non-directory answers warn before
                    // Launch. Permissions, busy workers and timeouts say nothing.
                    let problem = response
                        .err()
                        .and_then(|message| FolderProblem::recognize(&message))
                        .filter(|problem| *problem != FolderProblem::Unusable);
                    folder_check_result
                        .set(problem.map(|problem| (authority, problem, host_label)));
                }
                folder_check_completions.with_mut(|count| *count = count.wrapping_add(1));
                folder_check_pending.set(false);
            });
        }
    });
    let folder_notice = use_memo(move || {
        // Managed mode displays its preview, while the ordinary folder editor
        // remains dormant in the draft. Its old refusal cannot describe that
        // preview, and this subscription retires the marking on a mode switch.
        if destination_draft().is_managed() {
            return None;
        }
        // `snapshot()` deliberately subscribes to nothing; these field memos
        // are what make an edit of the folder re-check the marking.
        let _ = (cwd(), cwd_edited());
        let path = draft.snapshot().submitted_cwd();
        if let Some(refusal) = folder_refusal()
            && live_destination().as_ref() == Some(&refusal.target)
            && live_browse_connection() == refusal.connection
            && path == refusal.cwd
        {
            return Some(refusal.problem.notice(&refusal.host));
        }
        let (authority, problem, host) = folder_check_result()?;
        let current = folder_check_live().map(|(claim, _)| claim);
        browse_reply_is_current(
            &current,
            &authority,
            live_destination(),
            live_browse_connection(),
            &path,
        )
        .then(|| problem.notice(&host))
    });
    // History is shared helm state. Its revision may change because another
    // browser launched a session, so its resource must react both to the
    // live destination and to the feed rather than to this render's host
    // snapshot.
    let history_reader = use_signal(SurfaceReader::default);
    let launch_history = use_signal(|| None::<(CreateTarget, api::LaunchHistory)>);
    let history_unreadable = use_signal(|| None::<CreateTarget>);
    let history_feed_base = base.clone();
    use_feed_reader(move || {
        request_launch_history(
            history_feed_base.clone(),
            create_target,
            history_reader,
            launch_history,
            history_unreadable,
            Trigger::Notice,
        );
    });
    let history_base = base.clone();
    let requested_history_target = create_target();
    use_effect(use_reactive((&requested_history_target,), move |_| {
        request_launch_history(
            history_base.clone(),
            create_target,
            history_reader,
            launch_history,
            history_unreadable,
            Trigger::Explicit,
        );
    }));
    // Configuration edits also invalidate checkout previews. When the feed
    // is unavailable, unchanged host/list replies cannot tell this reader
    // about those edits. The component owns this fallback's lifetime; the
    // shared scheduled trigger neither queues behind a busy read nor cuts
    // short its backoff, and the feed gate withdraws it under build skew.
    let history_fallback_base = base.clone();
    use_future(move || {
        let base = history_fallback_base.clone();
        async move {
            loop {
                fallback_sleep().await;
                if fallback_polls_now() {
                    request_launch_history(
                        base.clone(),
                        create_target,
                        history_reader,
                        launch_history,
                        history_unreadable,
                        Trigger::Scheduled,
                    );
                }
            }
        }
    });
    // A feed update is fetched immediately, but it is not allowed to rewrite
    // the suggestion surface beneath an open draft. The offered snapshot is
    // promoted only by a deliberate picker/search transition below. A new
    // destination is the exception: its old suggestions lose authority at
    // once and remain blank until a reply for the new installation arrives.
    let mut offered_history = use_signal(|| None::<(CreateTarget, api::LaunchHistory)>);
    let history_for_offer = launch_history();
    // Feed replies update this handoff immediately, while `offered_history`
    // remains stable until a deliberate choice calls the shared promoter.
    let mut fetched_history = use_signal(|| None::<(CreateTarget, api::LaunchHistory)>);
    // Environment history is helm-wide reusable intent, not evidence about
    // the selected installation's default folder. It shares the promotion
    // boundary while retaining its own initial/pending/failed-read semantics.
    let fetched_environment_history = use_signal(|| None::<Vec<api::LaunchHistoryEntry>>);
    let offered_environment_history = use_signal(|| None::<Vec<api::LaunchHistoryEntry>>);
    // Independent read completion must be observable without mistaking a
    // simultaneous ordinary-history reply for consumption of environment data.
    let fetched_environment_history_revision = use_signal(|| 0_u64);
    // This revision is deliberately exposed on the mounted dialog for the
    // browser contract tests. Offered history must remain unchanged when this
    // advances; the attribute lets those tests prove the component consumed a
    // particular fresh reply before they inspect that frozen offer.
    let mut fetched_history_revision = use_signal(|| 0_u64);
    if *fetched_history.peek() != history_for_offer {
        fetched_history.set(history_for_offer.clone());
        fetched_history_revision.with_mut(|revision| *revision = revision.wrapping_add(1));
    }
    observe_environment_history(
        environment_history.read().as_ref(),
        fetched_environment_history,
        offered_environment_history,
        fetched_environment_history_revision,
    );
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
    let busy = ops.busy();

    // A host installation change rotates the idempotency key and any
    // host-specific refusal; the launch draft survives it.
    let bound_target = use_signal(|| None::<CreateTarget>);
    // Which prefill GENERATION (`CreatePrefill`) this form has already
    // applied. Compared by generation rather than by mere presence in the
    // effect below, because `prefill` stays populated at its latest
    // generation for as long as this form is open, so presence alone would
    // reseed on every unrelated rerun of that effect (a host reconnect, a
    // catalog refresh) for as long as a clone is on screen.
    let prefill_applied = use_signal(|| None::<u64>);
    // Initial search acceptance follows this effect, so mount-time host and
    // clone reconciliation cannot overwrite the action it applies.
    let mount_seeded = use_signal(|| false);
    let source_generation = prefill
        .as_ref()
        .map(|prefill| prefill.generation)
        .unwrap_or(0);
    let inherited_sandbox = (!host_fixed)
        .then(|| {
            let id = prefill
                .as_ref()
                .and_then(|prefill| prefill.host)
                .or_else(|| open_host.as_ref().map(|host| host.id))?;
            hosts
                .iter()
                .find(|host| host.id == id && host.session_owned)
                .and_then(|host| host.environment_target.clone())
        })
        .flatten();
    let mut sandbox_seeded = use_signal(|| None::<u64>);
    let seed_new_sandbox = prefill.is_none();
    use_effect(use_reactive(
        (
            &hosts_loaded,
            &source_generation,
            &inherited_sandbox,
            &origin_unresolved,
        ),
        move |(hosts_loaded, source_generation, inherited_sandbox, origin_unresolved)| {
            if seed_new_sandbox
                && hosts_loaded
                && mount_seeded()
                && !origin_unresolved
                && *sandbox_seeded.peek() != Some(source_generation)
            {
                if draft.snapshot().recipe().is_none()
                    && chosen_host.peek().is_none()
                    && let Some(recipe) = &inherited_sandbox
                {
                    draft.choose_target(Some(recipe.clone()));
                }
                sandbox_seeded.set(Some(source_generation));
            }
        },
    ));
    // The folder default as it stood when the current permission warning
    // appeared, keyed by its generation, and the copy a permission Undo hands
    // back; see `FolderDefaultInputs::restored`.
    let mut default_at_switch = use_signal(|| None::<(u64, FolderDefault)>);
    let mut restoring_default = use_signal(|| None::<FolderDefault>);
    let mut folder_default = use_signal(move || FolderDefault {
        pending: !opened_with_source_folder,
        ..FolderDefault::default()
    });
    // A launch in flight owns the draft it sent (its retry key and body), so
    // the default waits; launching itself is refused while it is pending.
    // Capture before this render's step: the switch that raised the warning
    // happened in an event handler, so the state is still the one it saw.
    if let Some(generation) = permission_escalation
        .read()
        .as_ref()
        .map(|notice| notice.generation)
        && default_at_switch
            .peek()
            .as_ref()
            .map(|(captured, _)| *captured)
            != Some(generation)
    {
        default_at_switch.set(Some((generation, folder_default.peek().clone())));
    }
    // The parent-derived target lags the form's own host choice by an effect
    // (see `live_destination`). Judge host changes and restores only once the
    // two agree, so a transient target is never taken for a host choice and a
    // permission Undo's handoff waits for the host it restored.
    let target = create_target();
    if !restart_mode && !busy && target == destination_now {
        let prefill_generation = *prefill_applied.read();
        let (next, seed) = folder_default_step(
            folder_default.peek().clone(),
            FolderDefaultInputs {
                target: target.as_ref(),
                prefill: prefill_generation,
                // Without a source, every host change is the person's; with
                // one, only once they took the host over from the clone's binding.
                host_is_users: prefill_generation.is_none()
                    || *clone_host_state.read() == CloneHostState::UserTookOver,
                selected: selected_folder.as_ref(),
                offered: offered_history.peek().as_ref(),
                unreadable: history_unreadable.read().as_ref(),
                restored: restoring_default.peek().as_ref(),
            },
        );
        if restoring_default.peek().is_some() {
            restoring_default.set(None);
        }
        // The seeded folder is this target's own default, so an inherited
        // folder's installation claim no longer describes it.
        if let Some(folder) = seed
            && draft.seed_default_folder(&folder)
            && remembered_destination.peek().is_some()
        {
            let mut remembered_destination = remembered_destination;
            remembered_destination.set(None);
        }
        if *folder_default.peek() != next {
            folder_default.set(next);
        }
    }
    // Whether Launch must wait for the host's default folder: only while one is
    // pending for a folder destination nobody chose.
    let folder_default_waiting = !restart_mode && folder_default.read().pending && {
        let snapshot = draft.snapshot();
        !snapshot.folder_is_explicit()
            && matches!(snapshot.destination(), DestinationDraft::Existing { .. })
    };
    let mut pending_initial_action = use_signal(move || initial_action);
    let mut pending_initial_templates =
        use_signal(move || std::collections::VecDeque::from(initial_templates));
    // This opening's explanation survives automatic replay, then gives way
    // to the user's edits. It is not part of the remembered launcher draft.
    let has_partial_template = partial_template_banner.is_some();
    let mut partial_template_hint = use_signal(move || has_partial_template);
    let choice_targets = ChoiceTargets {
        partial_template_hint,
        offered_history,
        create_target,
        fetched_history,
        offered_environment_history,
        fetched_environment_history,
    };
    // Handles only: each concrete operation earns its existing tracked reads
    // after live admission, rather than capturing authority at render time.
    let placement_targets = PlacementTargets {
        choices: choice_targets,
        live_destination,
        remembered_destination,
        clone_host_state,
        history_activation_attempts,
        preview_revision,
        browse_generation,
        browse_request,
        browse_result,
        browse_error,
    };
    let repository_targets = RepositoryTargets {
        open: repository_open,
        index: repository_index,
        search: composer_search,
        search_open: composer_search_open,
    };
    let folder_targets = FolderTargets {
        open: folder_open,
        active: folder_active,
    };
    let mut partial_hint_timer_started = use_signal(|| false);
    let partial_template_text = partial_template_banner
        .as_ref()
        .map(|banner| banner.text())
        .unwrap_or_default();
    // Cloned rather than borrowed into the effect below: `hosts` is this
    // component's own prop (not a `Signal`, so it cannot be `Copy`-captured
    // the way the surrounding signals are), and the render body further
    // down needs its own, unmoved copy for the selector.
    let hosts_for_reseed = hosts.clone();
    // Preserve the effect's generation trigger and the owner's one tracked
    // target read. Construct aliases here; host resolution must not precompute
    // or replace the entry target used by this invocation's final comparison.
    let source_targets = SourceTargets {
        create_target,
        bound_target,
        remembered_destination,
        prefill_applied,
        mount_seeded,
        partial_template_hint,
        auto_launch_pending,
        quick_launch_hidden,
        pending_initial_action,
        pending_initial_templates,
        clone_host_state,
        preview_revision,
        browse_generation,
        browse_request,
        browse_result,
        browse_error,
        error,
        preparation_error,
        composer_reset_reason,
        permission_escalation,
        template_panel,
    };
    use_effect(use_reactive(
        (&prefill.as_ref().map(|prefill| prefill.generation),),
        move |_| {
            draft.reconcile_source(
                prefill.as_ref(),
                hosts_loaded,
                &hosts_for_reseed,
                source_targets,
            );
        },
    ));

    // Only the active creation surface determines the notice; the other
    // surface retains a draft that may describe a different harness. A typed
    // command is never classified as Cursor here.
    let cursor_launch =
        launch_tab() == LaunchTab::Agent && structured_harness() == Some(LaunchHarness::Cursor);
    let displayed_invocation = invocation.read().clone();
    let command_snapshot = draft.snapshot();
    let restart_command = match command_snapshot.restart_edit() {
        Some(api::RestartEdit::Command(command)) if restart_mode => Some(command),
        _ => None,
    };
    let restart_command_problem = restart_command
        .as_ref()
        .and_then(|command| command.validate().err())
        .map(crate::local_validation_notice);
    let (command_changed, resume_changed, yolo_changed) =
        match (&restart_command, &restart_baseline) {
            (Some(command), Some(api::RestartEdit::Command(stored))) => (
                command.command != stored.command,
                command.resume != stored.resume,
                command.yolo != stored.yolo,
            ),
            _ => (false, false, false),
        };
    let restart_command_readonly = restart_mode
        && !command_snapshot.invocation_edited()
        && command_snapshot
            .invocation_raw_seed()
            .is_some_and(|raw| raw.chars().any(farhelm_proto::text::is_presentation_unsafe));
    let restart_resume_readonly = restart_mode
        && !command_snapshot.command_resume_edited()
        && command_snapshot
            .command_resume_raw_seed()
            .is_some_and(|raw| raw.chars().any(farhelm_proto::text::is_presentation_unsafe));

    let preview_agent_now = move || preview_agent_identity(draft);
    // Configuration epochs are global and monotonic. Retain the highest
    // observed value while a history refresh is pending or fails; falling
    // back to zero would authorize a preview we already know is obsolete.
    if let Some((target, history)) = &history_for_offer
        && Some(target) == create_target().as_ref()
        && history.checkout_config_revision > *observed_checkout_revision.peek()
    {
        observed_checkout_revision.set(history.checkout_config_revision);
    }
    // Reset before constructing authority so the preview and every later
    // submit see the same name on a host or repository change. Use the
    // synchronous destination claim: create_target is reconciled by an effect
    // and can still describe the prior host during this render.
    let _ = destination_draft();
    draft.retarget_checkout_name(destination_now.clone());
    // Display, preview authority, retry binding, submit, draft snapshot and
    // request-key re-read must share this policy. A stray raw-title read
    // would make the create disagree with its preview and be refused as stale.
    // Request assembly is plain; callers retain these exact subscriptions
    // even when an early refusal prevents a complete binding from existing.
    let observe_title_fields = move || {
        let _ = (
            destination_draft(),
            clone_checkout_naming(),
            title_raw_seed(),
            title(),
            title_edited(),
        );
    };
    let title_for_launch = move || {
        observe_title_fields();
        draft.snapshot().effective_title()
    };
    let mut proposed_authority = destination_draft().repo().and_then(|_| {
        let host = hosts.iter().find(|host| Some(host.id) == selected)?;
        observe_title_fields();
        checkout_preview_authority(
            host,
            &draft.snapshot(),
            observed_checkout_revision(),
            preview_agent_now(),
        )
    });
    let mut previous_authority = live_preview_authority.peek().clone();
    if let Some(previous) = &mut previous_authority {
        previous.generation = 0;
    }
    let revision_now = preview_revision();
    let mut applied_preview_revision = use_signal(|| 0_u64);
    if proposed_authority != previous_authority || revision_now != *applied_preview_revision.peek()
    {
        let generation = preview_generation
            .peek()
            .checked_add(1)
            .expect("preview generation exhausted");
        preview_generation.set(generation);
        applied_preview_revision.set(revision_now);
        if let Some(authority) = &mut proposed_authority {
            authority.generation = generation;
        }
        live_preview_authority.set(proposed_authority);
        if draft.snapshot().destination().repo().is_some() {
            // Semantic selection stays put. Same-repo reselection reaches this
            // boundary through preview_revision, and submit checks its applied
            // revision before an old Ready offer can be used before rerender.
            preview_state.set(PreviewState::Pending);
        }
    }
    let preview_base = base.clone();
    let preview_response = use_resource(move || {
        let authority = live_preview_authority();
        let base = preview_base.clone();
        async move {
            let authority = authority?;
            let result = api::preview_github_checkout(
                &base,
                authority.host.parse().expect("host id came from registry"),
                authority.incarnation,
                &authority.repo.identifier(),
                authority.title.as_deref(),
            )
            .await;
            Some((authority, result))
        }
    });

    // `None` until a catalog is in hand: still loading, or the read failed.
    // Compatibility checks take this rather than `catalog_models`, because an
    // empty list stands for "no models" there, not "not known yet"
    // (`launch_composer::selection_fits_catalog`).
    let catalog_answer = launch_catalog
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .cloned();
    let catalog_models = catalog_answer.clone().unwrap_or_default();
    let catalog_error = launch_catalog
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .cloned();
    let current_history_target = create_target();
    let recent_history = offered_recent_history(
        offered_history.read().as_ref(),
        current_history_target.as_ref(),
        destination_now.as_ref(),
        offered_environment_history
            .read()
            .as_deref()
            .unwrap_or_default(),
    );
    let folder_recents = folder_suggestions(&recent_history.folders, &cwd(), cwd_edited())
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let folder_choices = folder_recents
        .iter()
        .map(|folder| FolderChoice::Recent {
            // The callback owns an operational path, not its visible spelling.
            cwd: folder.display_cwd.raw().to_owned(),
            target: current_history_target.clone(),
        })
        .chain([FolderChoice::Home, FolderChoice::LocalHome])
        .collect::<Vec<_>>();
    let folder_choices_for_keys = folder_choices.clone();
    let folder_highlight = folder_active().filter(|index| *index < folder_choices.len());
    // Rank only combinations compatible with every explicit choice. Activating
    // one still replaces the whole draft; filtering decides which historical
    // combinations are offered, not which fields activation may write.
    let recent_filter = crate::launch_composer::ComposerFilter {
        harness: structured_harness(),
        model: structured_model(),
        effort: structured_effort(),
        // Not a bare `structured_permissions()` read: see
        // `structured_permissions_is_explicit`'s own doc for why the
        // passive memory seed must not act as a filter dimension.
        permissions: if structured_permissions_is_explicit() {
            structured_permissions()
        } else {
            None
        },
    };
    let recent_launches = crate::launch_composer::destination_recents(
        &recent_history,
        &recent_filter,
        &destination_draft(),
    )
    .into_iter()
    .cloned()
    .collect::<Vec<_>>();
    let selected_host_label = environment_target()
        .map(|target| {
            format!(
                "new {}",
                crate::environment_ui::target_label(&target, &environment_places)
            )
        })
        .unwrap_or_else(|| {
            selected
                .and_then(|id| hosts.iter().find(|host| host.id == id))
                .map(HostOption::label)
                .unwrap_or_else(|| "Unavailable host".to_string())
        });
    // Confirmation owns raw peer text and escapes it at its own rendering
    // boundary. Keep that snapshot separate from this form's display label.
    let host_label_for_restart = if restart_mode {
        restart_source
            .as_ref()
            .and_then(|source| source.host_name.as_ref())
            .map(|name| name.raw().to_owned())
            .unwrap_or(selected_host_label)
    } else {
        selected_host_label
    };
    let selected_host_label = if restart_mode {
        display_peer(&host_label_for_restart)
    } else {
        host_label_for_restart.clone()
    };
    // The host selector still shows unavailable destinations so a user can
    // see what changed, but an agent choice must not make Launch look ready
    // when the helm already knows that destination cannot accept it.
    let selected_host_suspended =
        selected.is_some_and(|id| hosts.iter().any(|host| host.id == id && host.suspended));
    let own_resume_host = resuming_host
        .read()
        .as_ref()
        .map(|preparation| preparation.host);
    // This is presentation only. In particular, a completed resume receipt
    // never makes Launch or its automatic-submit readiness available sooner.
    let hide_lagging_host_note = resume_display_handoff
        .read()
        .as_ref()
        .is_some_and(|receipt| Some(receipt) == displayed_resume.as_ref());
    let selected_host_available = recipe().is_some()
        || restart_host_confirmed
            && selected.is_some_and(|id| {
                hosts
                    .iter()
                    .find(|host| host.id == id)
                    .is_some_and(|host| host.available_for_launch(own_resume_host))
            });
    // A stored structured snapshot is provenance, rather than a promise that
    // a later release still supports every combination it named. Keep its
    // model and effort visible for clone, but refuse to turn an incompatible
    // known choice into a different launch by guessing a replacement. Pi's
    // omitted-permission compatibility rule was already applied while seeding.
    let invalid_permission_flags = launch_tab() == LaunchTab::Agent
        && structured_permissions() == Some(LaunchPermission::Custom)
        && farhelm_proto::split_permission_flags(&structured_permission_flags()).is_err();
    let structured_choice_error = structured_harness().and_then(|harness| {
        // Bad editor text is not evidence that the host's catalog changed.
        // Preserve that distinction beside the controls as well as at Launch.
        if invalid_permission_flags {
            return Some(CUSTOM_PERMISSION_FLAGS_REFUSAL);
        }
        let selection = draft.snapshot().editing_selection(harness);
        (!crate::launch_composer::selection_fits_catalog(&selection, catalog_answer.as_deref())).then_some(
            "This saved choice is no longer supported by the current catalog; choose a compatible model or effort",
        )
    });
    // Peer-owned values remain separate directional runs. The launch context
    // isolates its host and folder, while the summary isolates its model, so a
    // strong RTL value cannot reorder the punctuation around another value.
    //
    // The harness is named only on the agent tab. Switching to the command
    // tab deliberately keeps the agent draft (so a trip there and back
    // loses nothing), but a command launch runs the typed command, never
    // that harness — so the button must not promise "Codex" while the click
    // would launch something else.
    let launch_harness = if *launch_tab.read() == LaunchTab::Agent {
        structured_harness()
            .map(|harness| crate::launch_composer::harness_label(harness).to_string())
    } else if restart_mode {
        command_agent().map(|harness| crate::launch_composer::harness_label(harness).to_string())
    } else {
        None
    };
    // Recent setup hints keep their existing compact verb. The footer uses
    // the same operation with a sentence-case label and a separate target
    // summary, so replacement and a stop-first restart remain explicit.
    let submit_verb = if restart_mode {
        if restart_stop_first {
            "Stop and restart"
        } else {
            "Restart"
        }
    } else if is_replace_with {
        "Replace"
    } else {
        "Launch"
    };
    // A recent setup's accessible name replaces its visible content, hint
    // included, so in Replace with the name carries the verb itself.
    let recent_name_prefix = if is_replace_with { "Replace: " } else { "" };
    // What pressing "replace" stops, drawn from one render: the warning under
    // the button, and the precondition a press or answer drawn by this render
    // passes to the submit (see `replace_with_warning`).
    let (replace_warning, replace_guard) = if is_replace_with {
        replace_with_warning(replace_source_state.as_ref())
    } else {
        (None, crate::DeleteGuard::NothingAlive)
    };
    // Keep the command arm's conditional subscriptions. The live owner supplies
    // values after those reads, including seeds that never subscribed here.
    let current_launch = {
        if launch_tab() == LaunchTab::Command {
            let _ = (
                invocation(),
                invocation_edited(),
                command_yolo(),
                command_agent(),
                command_resume_on(),
                command_resume(),
                command_resume_edited(),
            );
        }
        draft.snapshot().launch_intent()
    };
    let retry_binding = current_launch.and_then(|launch| {
        let _ = cwd();
        observe_title_fields();
        let binding = draft.snapshot().request(RequestContext {
            host: hosts
                .iter()
                .find(|host| Some(host.id) == selected)
                .map(|host| (host.id, host.incarnation.as_str())),
            launch,
            replace_source: prefill_for_submit
                .as_ref()
                .and_then(|prefill| prefill.mode.replace_source()),
            checkout: None,
        })?;
        let destination = destination_draft();
        let repo = destination.repo()?;
        let installation = hosts
            .iter()
            .find(|host| Some(host.id) == selected)?
            .identity
            .as_deref()?;
        github_attempt()
            .filter(|(original, _)| same_fresh_intent(original, &binding, repo, installation))
            .map(|(binding, _)| binding)
    });
    // A preview cannot prove that a lost create was refused: its occupied
    // directory may belong to the request we are still reconciling. Derive
    // this from the same intent match that preserves its body and key.
    use_effect(use_reactive(&retry_binding, move |retry_binding| {
        let Some(Some((authority, result))) = preview_response.read().clone() else {
            return;
        };
        let Some(live) = live_preview_authority.peek().clone() else {
            return;
        };
        if authority != live {
            return;
        }
        let next_preview = match result {
            Ok(preview) if authority.accepts(&live, &preview) => PreviewState::Ready { authority: authority.clone(), preview },
            Ok(_) => PreviewState::Failed { authority: authority.clone(), message: "The preview is stale or belongs to a different host installation; select the repository again".into() },
            Err(failure) => {
                // This is the existing system edit: authoritative occupancy may
                // advance a default while busy, but never retire a retry key.
                if draft.advance_checkout_name_for_occupied_reply(
                    &authority, &live, &failure, retry_binding.is_some(),
                ) {
                    return;
                }
                PreviewState::Failed { authority: authority.clone(), message: failure.message }
            },
        };
        if draft.snapshot().destination().repo() == Some(&authority.repo) {
            // A completion changes observation only, after the authority and
            // repository fences above. It cannot select or restore a repo.
            preview_state.set(next_preview);
        }
    }));
    let current_preview = match destination_draft() {
        DestinationDraft::Github { .. } => match preview_state() {
            PreviewState::Ready { authority, preview }
                if live_preview_authority
                    .peek()
                    .as_ref()
                    .is_some_and(|live| authority.accepts(live, &preview)) =>
            {
                Some(preview)
            }
            _ => None,
        },
        _ => None,
    };
    let fresh_destination_ready = recipe().is_some()
        || matches!(destination_draft(), DestinationDraft::Existing { .. })
        || retry_binding.is_some()
        || current_preview.is_some()
        || (selected_host_suspended
            && matches!(destination_draft(), DestinationDraft::Github { .. }));
    let command_requires_text = retry_binding.is_none();
    let launch_prerequisites = LaunchPrerequisites {
        busy,
        saving_template: template_panel().is_some() || template_saving(),
        host_available: selected_host_available,
        destination_ready: fresh_destination_ready,
        github_search: matches!(destination_draft(), DestinationDraft::GithubInput { .. })
            || crate::launch_composer::scoped_query(&composer_search()).0
                == crate::launch_composer::SearchScope::Github,
        remembered_destination_valid: recipe().is_some() || remembered_destination_valid,
        folder_default_waiting: recipe().is_none()
            && retry_binding.is_none()
            && folder_default_waiting,
        needs_harness: retry_binding.is_none()
            && launch_tab() == LaunchTab::Agent
            && structured_harness().is_none(),
        invalid_permission_flags: retry_binding.is_none() && invalid_permission_flags,
        incompatible_choice: retry_binding.is_none()
            && launch_tab() == LaunchTab::Agent
            && structured_choice_error.is_some(),
        invalid_wrapper: launch_tab() == LaunchTab::Agent
            && wrapper_words(&structured_wrapper()).is_err(),
    };
    let container_refusal = draft.snapshot().container_refusal();
    let sandbox_reason = environment_target()
        .and_then(|target| {
            let name = target.recipe().to_string();
            match &sandbox_answer {
                None => Some("Loading recipes…".into()),
                Some(Err(error)) => Some(error.clone()),
                Some(Ok((recipes, settings))) => recipes
                    .iter()
                    .find(|recipe| recipe.name == name)
                    .map(|recipe| {
                        crate::environment_ui::ProviderUi(target.provider())
                            .recipe_refusal(recipe, settings)
                    })
                    .unwrap_or_else(|| Some(format!("Recipe {name:?} is unavailable"))),
            }
        })
        .or_else(|| {
            environment_target().and_then(|target| {
                target.place().host().and_then(|_| {
                    environment_places
                        .iter()
                        .find(|place| place.place == target.place())
                        .map(|place| place.reason.clone())
                        .unwrap_or_else(|| Some("The container host is unavailable".into()))
                })
            })
        });
    let launch_refusal = if origin_unresolved {
        Some("The source host's placement is unavailable; wait for Hosts to refresh or choose a target explicitly")
    } else if restart_mode && !restart_host_confirmed {
        Some("The source host installation cannot be confirmed; close and reopen restart with")
    } else if restart_mode && !selected_host_available {
        Some("The source host is unavailable; wait for it to reconnect before restarting")
    } else {
        launch_prerequisites.refusal()
    }.or(sandbox_reason.as_deref()).or(container_refusal.as_deref()).or_else(|| {
        if !restart_mode { return None; }
        let current = draft.snapshot();
        if model_open() && !model_draft().is_empty() || model_draft_error().is_some() {
            Some("Choose a model before restarting")
        } else if conversation_required && current.conversation().is_empty() {
            Some("Enter a conversation to restart")
        } else if current.restart_edit().as_ref() == restart_baseline.as_ref() {
            Some("Change a setting before restarting")
        } else if matches!(current.restart_edit(), Some(api::RestartEdit::Command(ref command)) if command.validate().is_err()) {
            Some("Correct the command before restarting")
        } else { None }
    });
    // A suspended destination promises allocation after Resume, not a path
    // offered by the previous connection. Hide it in every summary/field too.
    let allocate_after_resume = (recipe().is_some() || selected_host_suspended)
        && matches!(destination_draft(), DestinationDraft::Github { .. });
    let allocation_notice = if draft.snapshot().effective_title().is_empty() {
        "The folder is picked automatically when the host is ready"
    } else {
        "The folder is named after the session when the host is ready"
    };
    let displayed_preview = (!allocate_after_resume)
        .then(|| {
            retry_binding
                .as_ref()
                .and_then(|binding| {
                    binding
                        .github_checkout
                        .as_ref()
                        .map(|checkout| checkout.preview.clone())
                })
                .or(current_preview)
        })
        .flatten();
    let summary_folder = match destination_draft() {
        DestinationDraft::Existing { cwd } => display_peer(&cwd),
        DestinationDraft::GithubInput { query } if query.trim().is_empty() => {
            "Managed checkout (select a repository)".to_string()
        }
        DestinationDraft::GithubInput { query } => format!(
            "Managed checkout: {} (select a repository)",
            display_peer(&query)
        ),
        DestinationDraft::Github { .. } if allocate_after_resume => allocation_notice.into(),
        DestinationDraft::Github { repo, .. } => displayed_preview.as_ref().map_or_else(
            || format!("gh:{} (preview pending)", display_peer(&repo.identifier())),
            |preview| preview.cwd.escaped(),
        ),
    };
    // The editable `cwd` seed belongs to existing-folder mode. A fresh
    // checkout gets its path only from an accepted preview (or the retained
    // retry binding), so the destination field must not echo that old seed.
    let (folder_field_value, folder_placeholder, checkout_mode) = match destination_draft() {
        DestinationDraft::Existing { .. } => (cwd(), "", false),
        DestinationDraft::GithubInput { .. } => (String::new(), "Select a repository", true),
        DestinationDraft::Github { .. } if allocate_after_resume => {
            (String::new(), allocation_notice, true)
        }
        DestinationDraft::Github { .. } => (
            displayed_preview
                .as_ref()
                .map(|preview| preview.cwd.escaped())
                .unwrap_or_default(),
            if displayed_preview.is_some() {
                ""
            } else if matches!(preview_state(), PreviewState::Failed { .. }) {
                "Checkout preview unavailable"
            } else {
                "Waiting for checkout preview"
            },
            true,
        ),
    };
    // The name field never shows a title the launch will ignore: while a
    // fresh checkout drops the copied title (`copied_title_ignored`), the
    // field reads empty. Whenever a checkout launch is unnamed, the name it
    // will actually get, the preview's `repo-N`, is the placeholder instead.
    let title_ignored = copied_title_ignored(
        title_edited(),
        draft.snapshot().title_raw_seed().as_deref(),
        checkout_mode,
    );
    let _ = clone_checkout_naming();
    let clone_default = draft.snapshot().checkout_naming().default_title(
        title_edited(),
        draft.snapshot().title_raw_seed().as_deref(),
        destination_draft().repo(),
    );
    let title_field_value = if let Some(default) = clone_default {
        display_peer(&default)
    } else if title_ignored {
        String::new()
    } else {
        title()
    };
    let title_placeholder = if checkout_mode && (title_ignored || title().is_empty()) {
        displayed_preview
            .as_ref()
            .map(|preview| preview.basename.escaped())
            .unwrap_or_default()
    } else {
        String::new()
    };
    // A template keeps its base even while the field shows a resolved suffix.
    // This source marker disappears as soon as a hand edit takes over.
    let template_base = checkout_mode
        .then(|| {
            draft
                .snapshot()
                .checkout_naming()
                .template_name()
                .filter(|(_, kind)| *kind == farhelm_proto::launcher::TemplateNameKind::Base)
                .map(|(base, _)| {
                    (
                        base.to_string(),
                        draft.snapshot().checkout_naming().sequence.is_numbered(),
                    )
                })
        })
        .flatten();
    let summary_model = structured_model()
        .map(|model| display_peer(&model))
        .unwrap_or_else(|| "Default".to_string());
    let summary_prompt_lines = active_prompt(
        farhelm_proto::launcher::LauncherKind::Agent,
        structured_harness(),
        &prompt(),
    )
    .filter(|text| !text.is_empty())
    .map(|text| text.split('\n').count());
    let summary_effort = crate::launch_composer::effort_label(structured_effort());
    // Display names are independent from permission semantics: changing case
    // must not decide whether the footer paints the YOLO warning.
    let summary_permission_choice = match structured_harness() {
        Some(harness) => {
            crate::launch_composer::normalized_permissions(harness, structured_permissions())
        }
        None => structured_permissions(),
    };
    let summary_permission = summary_permission_choice
        .map(crate::launch_composer::permission_display_value)
        .unwrap_or("Default");
    let catalog_for_submit = catalog_answer.clone();
    let catalog_for_harness = catalog_answer.clone();
    // Pointer activation and Enter both apply exactly the same saved draft.
    // Enter deliberately submits only AFTER this callback returns: the form's
    // submit handler owns the intent key and operation lock, so it must remain
    // the one path that can mint a key for a launch.
    let apply_recent = Callback::<api::LaunchHistoryEntry, bool>::new({
        let catalog = catalog_models.clone();
        let history = history_for_recents.clone();
        let history_target = current_history_target.clone();
        move |entry: api::LaunchHistoryEntry| {
            draft.accept_recent(
                entry,
                &catalog,
                history_target.clone(),
                history.clone(),
                placement_targets,
                permission_escalation,
                composer_reset_reason,
            )
        }
    });
    let recovery_targets = RecoveryTargets {
        composer_reset_reason,
        clone_host_state,
        remembered_destination,
        model_draft,
        model_draft_error,
    };
    let model_targets = ModelTargets {
        reset_reason: composer_reset_reason,
        draft: model_draft,
        error: model_draft_error,
        open: model_open,
        active: model_active,
    };
    let search_targets = SearchTargets {
        placement: placement_targets,
        recovery: recovery_targets,
        template_error,
    };
    // Observe completed user transactions, not inner template edits: an
    // explicit template permission can replace an intermediate fallback.
    let note_permission_switch =
        use_callback(move |(before, automatic): (PermissionSwitchBefore, bool)| {
            note_completed_permission_switch(
                draft,
                before,
                automatic,
                permission_escalation,
                permission_escalation_generation,
                opening_generation,
            );
        });
    // Apply every catalog-row choice through one path so pointer and keyboard
    // activation cannot drift on reconciliation, error cleanup, or intent-key
    // invalidation. Custom drafts stay separate because they have no catalog
    // ownership to apply. Closing the list also discards the draft: the input
    // shows the selection once closed, and a draft that outlived the pick
    // would make the next Enter (the keystroke a person reaches for to
    // submit) apply the filter text as a custom id over the model just chosen.
    let apply_model_option = Callback::<crate::launch_composer::ModelOption>::new({
        let catalog_answer = catalog_answer.clone();
        move |option| {
            // Showing or hiding other harnesses' rows is a view toggle, not a
            // draft change: no busy gate, no intent-key invalidation.
            if let crate::launch_composer::ModelOption::ShowAll = option {
                model_show_all.set(!model_show_all());
                model_active.set(None);
                return;
            }
            match option {
                crate::launch_composer::ModelOption::HarnessDefault => {
                    draft.choose_default_model(choice_targets, model_targets);
                }
                crate::launch_composer::ModelOption::Model { id, harness } => {
                    let Some(before) = draft.choose_catalog_model(
                        id,
                        harness,
                        catalog_answer.as_deref(),
                        preferences,
                        choice_targets,
                        recovery_targets,
                    ) else {
                        return;
                    };
                    note_permission_switch.call((before, true));
                    model_draft_error.set(None);
                    model_draft.set(String::new());
                    model_open.set(false);
                    model_active.set(None);
                }
                crate::launch_composer::ModelOption::ShowAll => unreachable!("handled above"),
            }
        }
    });
    let active_search_harness = (*launch_tab.read() == LaunchTab::Agent)
        .then(&*structured_harness)
        .flatten();
    let active_search_model = (*launch_tab.read() == LaunchTab::Agent)
        .then(&*structured_model)
        .flatten();
    let search_text = composer_search();
    let (search_scope, search_repo_query) = crate::launch_composer::scoped_query(&search_text);
    let search_owns_repositories = search_scope == crate::launch_composer::SearchScope::Github;
    // One identity-bound discovery worker serves both repository entry points.
    // The dedicated field owns its query; global search takes priority only
    // while the user explicitly searches with gh:.
    // Matching keeps the raw inherited spelling. The escaped editor display
    // is never a query or a replacement for that selected identity.
    let repository_text = destination_draft().repository_text();
    let (repository_field_value, repository_read_only) = repository_field(&destination_draft());
    let repo_query = if search_owns_repositories {
        search_repo_query
    } else {
        &repository_text
    };
    let mut live_repository_authority = use_signal(|| None::<RepositoryAuthority>);
    let mut repository_generation = use_signal(|| 0_u64);
    // A selected repository still needs its host's first setup check, including
    // when Clone or a template supplies the selection before discovery runs.
    // Retain both answers for the exact host connection/installation and the
    // observed configuration epoch: false stops ordinary selected-repository
    // discovery, while true keeps setup visible. A newer epoch must check
    // again, since the effective folder may have been cleared elsewhere.
    let mut checkout_root_observation = use_signal(|| None::<(RepositoryAuthority, i64, bool)>);
    let host_root_observation = checkout_root_observation
        .read()
        .as_ref()
        .filter(|(claim, _, _)| {
            hosts.iter().any(|host| {
                Some(host.id) == selected
                    && host.id == claim.host
                    && host.connection == claim.incarnation
                    && host.identity.as_deref() == Some(claim.installation_identity.as_str())
            })
        })
        .map(|(_, revision, required)| (*revision, *required));
    // Freshness controls discovery, but a same-host editor must stay mounted
    // during that check: unmounting would discard its local draft and focus.
    // A current false answer retires it; changing host identity still hides it.
    let observed_root_required = host_root_observation
        .filter(|(revision, _)| *revision == observed_checkout_revision())
        .map(|(_, required)| required);
    let needs_checkout_root = host_root_observation.is_some_and(|(_, required)| required);
    // Unknown eligibility keeps the same discovery worker alive until the helm
    // answers. Query and destination authority still reject obsolete replies;
    // editing a selected repository's title cannot restart a configured scan.
    let choosing_repository =
        matches!(destination_draft(), DestinationDraft::GithubInput { .. }) || repository_open();
    let mut proposed_repository_authority = (choosing_repository
        || search_owns_repositories
        || (checkout_mode && observed_root_required != Some(false)))
    .then(|| {
        let host = hosts.iter().find(|host| Some(host.id) == selected)?;
        Some(RepositoryAuthority {
            generation: 0,
            host: host.id,
            incarnation: host.connection,
            installation_identity: host.identity.clone()?,
            destination_generation: preview_generation(),
            query: repo_query.to_string(),
        })
    })
    .flatten();
    let mut previous_repository_authority = live_repository_authority.peek().clone();
    if let Some(previous) = &mut previous_repository_authority {
        previous.generation = 0;
    }
    if proposed_repository_authority != previous_repository_authority {
        let generation = repository_generation
            .peek()
            .checked_add(1)
            .expect("search generation exhausted");
        repository_generation.set(generation);
        if let Some(authority) = &mut proposed_repository_authority {
            authority.generation = generation;
        }
        live_repository_authority.set(proposed_repository_authority);
    }
    let repository_base = base.clone();
    let mut repository_response = use_resource(move || {
        let authority = live_repository_authority();
        let base = repository_base.clone();
        async move {
            let authority = authority?;
            crate::reader::sleep_ms(150).await;
            let response = api::fetch_github_repositories(
                &base,
                authority.host,
                authority.incarnation,
                &authority.query,
            )
            .await;
            Some((authority, response))
        }
    });
    let repository_result =
        repository_response
            .read()
            .clone()
            .flatten()
            .and_then(|(authority, response)| {
                let live = live_repository_authority.peek().clone()?;
                if authority != live {
                    return None;
                }
                match response {
                    Ok(reply) if authority.accepts(&live, &reply) => Some(Ok(reply)),
                    Ok(_) => Some(Err(
                        "Repository suggestions belong to a different host installation"
                            .to_string(),
                    )),
                    Err(error) => Some(Err(error)),
                }
            });
    if let Some(Ok(reply)) = &repository_result {
        let observation = live_repository_authority.peek().clone().map(|claim| {
            (
                claim,
                observed_checkout_revision(),
                reply.needs_checkout_root,
            )
        });
        if *checkout_root_observation.peek() != observation {
            checkout_root_observation.set(observation);
        }
    }
    let show_checkout_root_setup = checkout_mode && needs_checkout_root;
    // This local note also holds UI literals and request errors. Keep the scan
    // reply raw here; both placements below escape the final note as before.
    // Request and scan failures are warnings; an incomplete search's ordinary
    // suggestion to enter owner/repo remains a hint.
    let repository_note =
        match &repository_result {
            Some(Ok(reply)) => reply
                .scan_error
                .as_ref()
                .map(|text| (text.raw().to_owned(), true))
                .or_else(|| {
                    reply.truncated.then(|| {
                        ("Repository search is incomplete; a valid owner/repo can still be selected"
                        .into(), false)
                    })
                }),
            Some(Err(message)) => Some((message.clone(), true)),
            None => None,
        };
    // Search and the visible host selector must offer the same registry
    // snapshot; the helper owns no separate host cache or name resolution.
    let composer_hosts = hosts
        .iter()
        .filter(|host| !host.session_owned)
        .map(|host| crate::launch_composer::ComposerHost {
            id: host.id,
            name: host.name.clone(),
            label: host.label(),
            local: host.local,
        })
        .collect::<Vec<_>>();
    let agent_without_harness = launch_tab() == LaunchTab::Agent && structured_harness().is_none();
    let mut search_rows = crate::launch_composer::search_results(
        agent_without_harness,
        &recent_history,
        &catalog_models,
        &composer_search(),
        active_search_harness,
        active_search_model.as_deref(),
    );
    search_rows.extend(crate::launch_composer::name_host_search_results(
        &composer_search(),
        &composer_hosts,
    ));
    if recipe().is_some() {
        search_rows.retain(|row| {
            !matches!(
                row,
                crate::launch_composer::ComposerSearchResult::Folder(_)
                    | crate::launch_composer::ComposerSearchResult::BrowsePath(_)
            )
        });
    }
    let templates_answer = launch_templates.read().clone();
    let templates_read_error = templates_answer
        .as_ref()
        .and_then(|answer| answer.as_ref().err())
        .cloned();
    let templates_now = templates_answer.and_then(Result::ok).unwrap_or_default();
    let template_names: Vec<String> = templates_now
        .iter()
        .map(|template| template.name.clone())
        .collect();
    search_rows.extend(crate::launch_composer::template_search_results(
        &composer_search(),
        &template_names,
    ));
    if search_scope == crate::launch_composer::SearchScope::Github {
        let discovered = repository_result
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map_or(&[][..], |r| r.repos.as_slice());
        search_rows.extend(
            repository_choices(repo_query, discovered)
                .into_iter()
                .map(crate::launch_composer::ComposerSearchResult::Github),
        );
    }
    let repository_suggestions = repository_choices(
        &repository_text,
        repository_result
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map_or(&[][..], |r| r.repos.as_slice()),
    );
    let repository_active = repository_index().min(repository_suggestions.len().saturating_sub(1));
    let repository_suggestions_for_keys = repository_suggestions.clone();
    let repository_query_for_keys = repository_text.clone();
    let repository_authority_for_keys = live_repository_authority.peek().clone();
    // The dedicated field uses the same owner/repo grammar as gh: search,
    // without any harness or catalog dependency.
    let repository_message = crate::launch_composer::empty_search_message(
        &format!("gh:{repository_text}"),
        false,
        false,
        false,
    );
    let search_rows = crate::launch_composer::finish_search_results(
        &composer_search(),
        search_rows,
        agent_without_harness,
    );
    let search_message = search_rows
        .is_empty()
        .then(|| {
            crate::launch_composer::empty_search_message(
                &composer_search(),
                agent_without_harness,
                launch_catalog.read().is_none(),
                launch_templates.read().is_none(),
            )
        })
        .flatten();
    let search_result_groups = crate::launch_composer::grouped_search_results(search_rows);
    let catalog_models_for_search_input = catalog_models.clone();
    let composer_hosts_for_search_input = composer_hosts.clone();
    let template_names_for_search_input = template_names.clone();
    let repositories_for_search_input = repository_result
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|reply| {
            live_repository_authority
                .peek()
                .clone()
                .map(|authority| (authority, reply.repos.clone()))
        });
    let search_query_for_keys = composer_search();
    let search_results_for_keys = search_result_groups
        .iter()
        .flat_map(|(_, results)| results.iter().cloned())
        .collect::<Vec<_>>();
    // The keyboard index is set by `oninput` against the results of THAT
    // keystroke, but the list it indexes is rebuilt on every render from
    // live state: a harness chosen from its segment while the list is open
    // rescopes the models and adds or removes the effort rows, and a
    // promoted history snapshot can change the custom-model rows. Clamping
    // at every read keeps `aria-activedescendant`, the highlighted row, and
    // Enter pointing at a row that exists, instead of naming an option id
    // that is not in the DOM and swallowing Enter.
    let composer_active_index =
        composer_search_index().min(search_results_for_keys.len().saturating_sub(1));
    let browse_base = base.clone();
    // This snapshot is used only to construct the outbound request. The
    // reply is checked against `browse_target`, which stays live while the
    // request is in flight.
    let browse_hosts = hosts.clone();
    let browse_target = create_target;
    let browse_base_for_folder = browse_base.clone();
    let browse_hosts_for_folder = browse_hosts.clone();
    let hosts_for_destination_choice = hosts.clone();
    // Every accepted search action owns its captured result. Promoting history
    // may change later suggestions, never the result this action applies.
    // Initial switcher choices use this same path, keeping template validation,
    // browse invalidation and field ownership identical to ordinary search.
    let hosts_for_template_save = hosts.clone();
    let host_label_for_template_save = selected_host_label.clone();
    let action_hosts = hosts.clone();
    let action_catalog = catalog_answer.clone();
    let action_recipes = recipe_names.clone();
    let shortcut_recipes = recipe_names.clone();
    let shortcut_hosts = hosts.clone();
    let shortcut_catalog = catalog_answer.clone();
    let action_history_target = current_history_target.clone();
    let action_browse_base = browse_base.clone();
    let action_browse_hosts = browse_hosts.clone();
    let question_open_host = open_host.clone();
    let question_prefill = prefill_for_submit.clone();
    let accept_search = use_callback(
        move |(result, captured, from_shortcut): (
            crate::launch_composer::ComposerSearchResult,
            Option<farhelm_proto::launcher::LaunchTemplate>,
            bool,
        )| {
            let Some(accepted) = draft.accept_search_result(
                result,
                captured.as_ref(),
                from_shortcut,
                SearchInputs {
                    recipes: &action_recipes,
                    templates: &templates_now,
                    hosts: &action_hosts,
                    catalog: action_catalog.as_deref(),
                    host_fixed,
                    history_target: action_history_target.clone(),
                    preferences,
                },
                search_targets,
            ) else {
                return false;
            };
            let switch_before = accepted.before;
            let automatic_permission = accepted.automatic_permission;
            let browse_path = accepted.browse_path;
            let is_template = accepted.is_template;
            // Reconcile only the completed template, after its prompt and full
            // retry pair are restored. Inner choices may temporarily differ.
            // All reads stay untracked even when this runs inside opening replay.
            if is_template && template_error.peek().is_none() && yolo_refusal.peek_key().is_some() {
                let retained = intent_key.peek().clone();
                let selected = effective_create_host(
                    &action_hosts,
                    *chosen_host.peek(),
                    question_open_host.as_ref(),
                );
                let current = retained.as_ref().and_then(|(_, refused)| {
                    completed_template_question_binding(
                        &draft.snapshot(),
                        &action_hosts,
                        selected,
                        create_target.peek().as_ref(),
                        question_prefill
                            .as_ref()
                            .and_then(|prefill| prefill.mode.replace_source()),
                        refused,
                    )
                });
                if retire_changed_template_question(
                    &mut yolo_refusal,
                    retained.as_ref(),
                    current.as_ref(),
                ) {
                    yolo_error.set(None);
                }
            }
            if let Some(path) = browse_path {
                request_directory_browse(
                    action_browse_base.clone(),
                    selected,
                    &action_browse_hosts,
                    browse_target,
                    path,
                    browse_generation,
                    browse_request,
                    browse_result,
                    browse_error,
                    browse_reply_completions,
                    live_browse_connection,
                    draft,
                );
            }
            note_permission_switch.call((switch_before, automatic_permission));
            if !from_shortcut {
                composer_search.set(String::new());
                composer_search_open.set(false);
                focus_composer_surface();
            }
            true
        },
    );
    // Selection is the same explicit intent transition as a gh: result,
    // including same-repository reselection after a refused preview.
    let select_repository = use_callback(move |repo: GithubRepo| {
        draft.select_repository(repo, placement_targets, repository_targets);
    });
    // Names need only mount seeding. Templates also need completed reads:
    // validating against temporary empty inputs would reject a valid choice.
    // While a template waits, the form presents a cancellable loading state
    // rather than accepting edits that the late application would overwrite.
    use_effect(use_reactive((&hosts_loaded,), move |(hosts_loaded,)| {
        let captured = pending_initial_templates.read().front().cloned();
        let action = captured
            .as_ref()
            .map(|template| {
                crate::launch_composer::ComposerSearchResult::Template(template.name.clone())
            })
            .or_else(&*pending_initial_action);
        let Some(action) = action else {
            return;
        };
        if !mount_seeded() {
            return;
        }
        let template_target = match captured.as_ref() {
            Some(template) => template.fields.launch_target(),
            None => launch_templates.read().as_ref().and_then(|answer| answer.as_ref().ok())
                .and_then(|templates| templates.iter().find(|template| {
                    matches!(&action, crate::launch_composer::ComposerSearchResult::Template(name) if name == &template.name)
                }))
                .and_then(|template| template.fields.launch_target()),
        };
        if matches!(
            action,
            crate::launch_composer::ComposerSearchResult::Template(_)
        ) && (!hosts_loaded
            || (captured.is_none() && launch_templates.read().is_none())
            || launch_catalog.read().is_none()
            || place_catalog_pending.call(template_target)
            || (captured.as_ref().is_some_and(|template| template.fields.launch_target().is_some())
                || draft.snapshot().recipe().is_some()
                || launch_templates.peek().as_ref().and_then(|answer| answer.as_ref().ok())
                    .is_some_and(|templates| templates.iter().any(|template| {
                        matches!(&action, crate::launch_composer::ComposerSearchResult::Template(name) if name == &template.name)
                            && template.fields.launch_target().is_some()
                    }))) && sandbox_catalog.read().is_none())
        {
            return;
        }
        if matches!(
            action,
            crate::launch_composer::ComposerSearchResult::Template(_)
        ) && captured.is_none()
            && let Some(Err(message)) = launch_templates.read().as_ref()
        {
            template_error.set(Some(format!(
                "Couldn't load templates: {message}; search tl: and retry"
            )));
            pending_initial_action.set(None);
            return;
        }
        if accept_search.call((action, captured.clone(), false)) {
            if captured.is_some() {
                if template_error.peek().is_some() {
                    // A refused template owns the fallback. Later templates
                    // must not clear its error or silently repair its fields.
                    pending_initial_templates.write().clear();
                } else {
                    pending_initial_templates.write().pop_front();
                }
            } else {
                pending_initial_action.set(None);
            }
        }
    }));
    // Shortcut batches use the same captured-template acceptance as an
    // opening, but never join its loading or auto-submit lifetime. Validate
    // a whole press before replay, then apply it without a render boundary
    // where a later refusal could strand earlier edits from that press.
    let mut shortcut_replay_checking = use_signal(|| false);
    use_effect(use_reactive((&hosts_loaded,), move |(hosts_loaded,)| {
        if shortcut_replay_checking() {
            return;
        }
        let Some(request) = shortcut_requests.read().front().cloned() else {
            return;
        };
        if request.opening != *opening_generation.peek()
            || ops.busy_now()
            || template_panel.peek().is_some()
            || template_saving()
        {
            shortcut_requests.write().pop_front();
            return;
        }
        if !mount_seeded()
            || pending_initial_action().is_some()
            || !pending_initial_templates.read().is_empty()
        {
            return;
        }
        if request.refusal.is_none()
            && (!hosts_loaded
                || launch_catalog.read().is_none()
                || launch_templates.read().is_none()
                || place_catalog_pending.call(environment_target())
                || request
                    .templates
                    .iter()
                    .any(|template| place_catalog_pending.call(template.fields.launch_target()))
                || (draft.snapshot().recipe().is_some()
                    || request
                        .templates
                        .iter()
                        .any(|template| template.fields.launch_target().is_some()))
                    && sandbox_catalog.read().is_none())
        {
            return;
        }
        // A batch can wait for host/catalog readiness or for earlier edits.
        // Check modal ownership after that wait; validated replay has no
        // await or render boundary where a later dialog could take over.
        shortcut_replay_checking.set(true);
        let shortcut_hosts = shortcut_hosts.clone();
        let shortcut_catalog = shortcut_catalog.clone();
        let shortcut_recipes = shortcut_recipes.clone();
        spawn(async move {
            let current = crate::launch_sequence::shortcut_context_snapshot().await;
            if request.opening != *opening_generation.peek()
                || ops.busy_now()
                || template_panel.peek().is_some()
                || template_saving()
                || !current.is_some_and(|snapshot| {
                    snapshot.context == "launcher"
                        && snapshot.modal_opening == request.context.modal_opening
                })
            {
                shortcut_requests.write().pop_front();
                shortcut_replay_checking.set(false);
                return;
            }
            if let Some(message) = request.refusal {
                template_error.set(Some(message));
                shortcut_requests.write().pop_front();
            } else if !request.templates.is_empty() {
                let templates = request.templates.into_iter().collect::<Vec<_>>();
                match validated_template_batch_known(
                    &templates,
                    &shortcut_hosts,
                    shortcut_catalog.as_deref().unwrap_or_default(),
                    host_fixed,
                    &draft.snapshot(),
                    *chosen_host.peek(),
                    &shortcut_recipes,
                ) {
                    Err(refusal) => template_error.set(Some(refusal)),
                    Ok(_) => {
                        // Each acceptance still makes the same form edits as
                        // `tl:`. No await or render splits this press in two.
                        for template in templates {
                            let action = crate::launch_composer::ComposerSearchResult::Template(
                                template.name.clone(),
                            );
                            if !accept_search.call((action, Some(template), true))
                                || template_error.peek().is_some()
                            {
                                break;
                            }
                        }
                        restore_shortcut_focus_if_lost();
                    }
                }
                shortcut_requests.write().pop_front();
            } else {
                shortcut_requests.write().pop_front();
            }
            shortcut_replay_checking.set(false);
        });
    }));
    // Pointer and keyboard choices share their transition callbacks. Enter
    // runs these synchronously before `launch_from_choice` submits, and the
    // submit reads the live draft.
    let choose_harness = EventHandler::<LaunchHarness>::new({
        let catalog = catalog_for_harness.clone();
        move |harness| {
            let Some(before) = draft.choose_harness(
                harness,
                catalog.as_deref(),
                preferences,
                choice_targets,
                recovery_targets,
            ) else {
                return;
            };
            note_permission_switch.call((before, true));
            focus_composer_surface();
        }
    });
    let choose_launch_tab = EventHandler::<LaunchTab>::new(move |tab| {
        if !draft.choose_launch_tab(tab, choice_targets) {
            return;
        }
        focus_composer_surface();
    });
    // No timer guesses when a template or managed-checkout preview is ready.
    // Each boundary is the same one the ordinary Launch button consumes. Once
    // submitted, this flag is spent even after an ambiguous reply or path race.
    let quick_target_ready = recipe().is_some()
        || target_matches_selection(selected, &hosts, create_target.read().as_ref());
    // Replay effects and auto-submit can run in the same effect batch. The
    // final replay must first earn a render with its resulting readiness and
    // refusal, rather than consume those values from before the last edit.
    let initial_setup_pending =
        pending_initial_action().is_some() || !pending_initial_templates.read().is_empty();
    // The effect needs readiness, not a borrow of this render's owned refusal
    // text. Compatibility refusals can produce a fresh String on every render.
    let launch_has_refusal = launch_refusal.is_some();
    let environment_place_pending = place_catalog_pending.call(environment_target());
    use_effect(use_reactive(
        (
            &hosts_loaded,
            &launch_has_refusal,
            &quick_target_ready,
            &busy,
            &selected_host_available,
            &host_read_error,
            &initial_setup_pending,
            &environment_place_pending,
        ),
        move |(
            hosts_loaded,
            has_refusal,
            target_ready,
            busy,
            host_available,
            host_error,
            setup_pending,
            place_pending,
        )| {
            if auto_launch_pending()
                && !hosts_loaded
                && let Some(message) = host_error
            {
                auto_launch_pending.set(false);
                quick_launch_hidden.set(false);
                preparation_error.set(Some(format!(
                    "Couldn't prepare quick launch: the host list failed: {message}"
                )));
                return;
            }
            if !auto_launch_pending()
                || !mount_seeded()
                || setup_pending
                || place_pending
                || pending_initial_action().is_some()
                || !pending_initial_templates.read().is_empty()
                || !hosts_loaded
            {
                return;
            }
            if busy {
                return;
            }
            // An offline host cannot produce a bound target or preview. It is
            // a fallback to show, rather than readiness to wait for invisibly.
            if !host_available {
                auto_launch_pending.set(false);
                launch_attempted.set(LaunchAttempt::Refused);
                quick_launch_hidden.set(false);
                return;
            }
            if !target_ready {
                return;
            }
            // Launch from the host's default folder, not from `~` before the
            // recent setups that decide it have arrived. If they could not be
            // read, show the launcher rather than launch into a fallback guess.
            let default = folder_default();
            let untouched_folder = {
                let snapshot = draft.snapshot();
                !snapshot.folder_is_explicit()
                    && matches!(snapshot.destination(), DestinationDraft::Existing { .. })
            };
            if recipe().is_none() && (default.pending || default.unread) && untouched_folder {
                if default.unread {
                    auto_launch_pending.set(false);
                    quick_launch_hidden.set(false);
                    preparation_error.set(Some(
                        "Couldn't prepare quick launch: this host's recent setups could not be read, so its last folder is unknown; check the folder and launch".to_string(),
                    ));
                }
                return;
            }
            let _ = destination_draft();
            let preview_pending = recipe().is_none()
                && match draft.snapshot().destination() {
                    DestinationDraft::Github { .. } => {
                        matches!(preview_state(), PreviewState::Pending)
                    }
                    _ => false,
                };
            if preview_pending {
                return;
            }
            auto_launch_pending.set(false);
            if has_refusal || template_error().is_some() {
                launch_attempted.set(LaunchAttempt::Refused);
                quick_launch_hidden.set(false);
                return;
            }
            automatic_submit_baseline.set(Some(quick_press_baseline));
            resubmit_quick_composer(*opening_generation.peek());
        },
    ));
    use_effect(move || {
        if quick_launch_hidden()
            && (error().is_some()
                || template_error().is_some()
                || yolo_refusal.is_open()
                || (launch_attempted().was_attempted() && !ops.busy_now()))
        {
            quick_launch_hidden.set(false);
        }
    });
    // Restore isolation after a submitted wait even if quick preparation stays
    // hidden; only an actual reveal can receive field focus. Ordinary rerenders
    // and later draft/refusal changes never reclaim the user's chosen control.
    use_effect(move || {
        let hidden = composer_hidden();
        let submitted = submitted_wait();
        let (was_hidden, was_submitted) = previous_presentation();
        if (hidden, submitted) != (was_hidden, was_submitted) {
            previous_presentation.set((hidden, submitted));
        }
        // Not keyed to a hidden transition: `release_submitted_composer` records
        // the hidden presentation before this render, so none would be seen.
        if hidden {
            blur_hidden_composer(mounted_opening);
        }
        if !submitted && (was_submitted || (was_hidden && !hidden)) {
            restore_composer_modal(
                mounted_opening,
                !hidden && !yolo_refusal.is_open(),
                if restart_mode {
                    FocusTarget::new(
                        ".create-session-submit:not([disabled]):not([aria-disabled=\"true\"])",
                        Some(".launch-composer-cancel:not([disabled])"),
                        false,
                    )
                } else {
                    composer_surface_target()
                },
            );
        }
    });
    // HostPicker owns transient browsing. The concrete owner operation retains
    // the synchronous history fence for callbacks queued before another render.
    let choose_host = EventHandler::new(move |id: HostId| {
        draft.choose_host(id, &hosts_for_destination_choice, placement_targets);
    });
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
                "The host you picked is no longer registered, so this create would go to the \
                 one selected now"
            }
        }
        if let Some(preparation) = resuming_host().filter(|preparation| Some(preparation.host) == selected) {
            HostResumeProgress { preparation }
        } else if selected.is_some() && !selected_host_available && !hide_lagging_host_note {
            div { class: "create-session-host-note",
                "The selected host is unavailable; choose a connected host before launching"
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
    // Pointer and keyboard choices share admission, browse invalidation and raw
    // reseeding. None submits. Equal ordinary paths keep unresolved identity
    // unless they correct an invalid installation claim. Local home is still
    // a deliberate host pin even when selected; home changes only its folder.
    let choose_folder = Callback::new(move |choice: FolderChoice| {
        draft.choose_folder(
            choice,
            local_host_id,
            local_destination.clone(),
            placement_targets,
            folder_targets,
        );
    });
    // This is the same deliberate permission transition for the row and the
    // guarded answer. Admission stays synchronous; a queued click must not
    // retire a request whose operation claim is already held.
    let choose_permissions = Callback::new(move |permission: Option<LaunchPermission>| {
        draft.choose_permissions(permission, choice_targets);
    });
    // A guarded answer may arrive after another control's event but before
    // that control's render. Reconstruct the executable draft from its live
    // owner rather than treating the rendered question as edit authority.
    let live_yolo_binding = {
        let hosts = hosts.clone();
        let open_host = open_host.clone();
        let prefill = prefill_for_submit.clone();
        move || {
            let current = draft.snapshot();
            if current.tab() != LaunchTab::Agent {
                return None;
            }
            current.harness()?;
            wrapper_words(&current.wrapper()).ok()?;
            let LaunchIntent::Structured(selection) = current.launch_intent()? else {
                return None;
            };
            let selected = effective_create_host(&hosts, *chosen_host.peek(), open_host.as_ref());
            if !target_matches_selection(selected, &hosts, create_target.peek().as_ref()) {
                return None;
            }
            observe_title_fields();
            let mut binding = current.request(RequestContext {
                host: hosts
                    .iter()
                    .find(|host| Some(host.id) == selected)
                    .map(|host| (host.id, host.incarnation.as_str())),
                launch: LaunchIntent::Structured(selection),
                replace_source: prefill
                    .as_ref()
                    .and_then(|prefill| prefill.mode.replace_source()),
                checkout: None,
            })?;
            match current.destination() {
                DestinationDraft::Existing { .. } => {}
                DestinationDraft::Github { repo, .. } => {
                    let authority = live_preview_authority.peek().clone()?;
                    if authority.repo != repo || authority.agent != preview_agent_now() {
                        return None;
                    }
                    let (_, refused) = intent_key.peek().clone()?;
                    let installation = hosts
                        .iter()
                        .find(|host| Some(host.id) == selected)?
                        .identity
                        .as_deref()?;
                    // The refused request owns this exact preview and cwd.
                    // Reconciliation may retain them across a newer offer;
                    // admission must compare that request, not replace it.
                    binding = bind_launcher_question_checkout(
                        &current,
                        binding,
                        &repo,
                        installation,
                        &refused,
                    )?;
                }
                DestinationDraft::GithubInput { .. } => return None,
            }
            Some(binding)
        }
    };
    // Native submit, choice and recent-row Enter, admitted guarded answers
    // and taken YOLO answers share this one Rust entry. Keeping transition,
    // admission and the synchronous operation claim in one turn avoids
    // desktop JavaScript evaluation dispatching a later draft. Every caller
    // passes the Replace precondition the press it acts for saw: a native
    // submit or Enter passes this render's, a guarded or YOLO answer its own
    // render's. Only the quick-launch auto-submit still goes through the
    // browser (`resubmit_quick_composer`).
    // The destructive consent belongs to the render that drew the action,
    // including deferred YOLO answers. It is never widened after an await.
    let submit_consent = (replace_guard, restart_stop_first, opening_generation());
    let submit_composer = Callback::<(
        crate::DeleteGuard,
        bool,
        u64,
        Option<u64>,
        Option<SubmitGuards>,
    )>::new({
        let hosts = hosts.clone();
        let open_host = open_host.clone();
        let prefill_for_submit = prefill_for_submit.clone();
        let base = base.clone();
        let catalog_for_submit = catalog_for_submit.clone();
        move |(
            replace_guard,
            restart_stop_first,
            rendered_opening,
            press_baseline,
            inherited_guard,
        ): (
            crate::DeleteGuard,
            bool,
            u64,
            Option<u64>,
            Option<SubmitGuards>,
        )| {
            let continued = inherited_guard.is_some();
            let mut held = inherited_guard;
            // Validation still captures the live draft synchronously. Its
            // early returns leave an inherited wake token here, so a refused
            // internal reentry can admit Return before releasing that token.
            // A branch takes it only when its actual future owns the next step.
            (|| {
                if rendered_opening != *opening_generation.peek() {
                    return;
                }
                // A queued duplicate cannot clear a completed error or
                // question while its first request still owes Return.
                if !continued && ops.busy_now() {
                    return;
                }
                if !continued
                    && guarded_submit_pending
                        .peek()
                        .as_ref()
                        .is_some_and(|(_, guard)| guard.is_some())
                {
                    return;
                }
                // Any ordinary submit supersedes an accepted press still waiting
                // for preview, including a submit refused before a request exists.
                guarded_submit_pending.set(None);
                // A queued submit can outlive the render that exposed Launch.
                // Check replay state live before clearing its loading reason or
                // taking a mutation claim for an incomplete initial setup.
                if pending_initial_action.peek().is_some()
                    || !pending_initial_templates.peek().is_empty()
                {
                    return;
                }
                // Parent actions revoke this before the render updates the
                // generation attribute. An old queued auto-submit cannot use
                // a newly supplied Clone/Replace draft in that gap.
                if *quick_launch_hidden.peek() && !*auto_launch.peek() {
                    auto_launch_pending.set(false);
                    quick_launch_hidden.set(false);
                    return;
                }
                launch_attempted.set(LaunchAttempt::Refused);
                error.set(None);
                preparation_error.set(None);
                if template_panel().is_some() || template_saving() {
                    return;
                }
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
                // Taken before anything can refuse this submit, so a request
                // to stop asking lives exactly as long as the one submit its
                // button started.
                let stop_asking_for = yolo_stop_asking.write().take();
                if held.is_none() {
                    held = ops.claim_guard().map(|launch| SubmitGuards {
                        launch,
                        source: None,
                    });
                    if held.is_none() {
                        return;
                    }
                }
                // The source belongs to this accepted submission before
                // any hidden wake/preview branch. Internal reentry carries
                // this exact owner instead of briefly releasing/reclaiming.
                if let Some(source) = prefill_for_submit
                    .as_ref()
                    .and_then(|prefill| prefill.mode.replace_source())
                {
                    let guards = held.as_mut().expect("submission owns its launch token");
                    if guards.source.is_none() {
                        let Some(claim) = session_operations.claim(source) else {
                            error.set(Some("Another action on this session is in progress".into()));
                            return;
                        };
                        session_operations.pending.write().insert(source.to_owned());
                        guards.source = Some((source.to_owned(), claim));
                    }
                }
                // The confirmation, if any, is matched against the key this
                // submit ends up using, below; the question itself is
                // answered either way. An answer already took it; a Launch
                // pressed while it is open supersedes it, which is
                // reconciliation rather than an answer, hence `clear`.
                yolo_refusal.clear();
                yolo_error.set(None);
                // Frozen HERE, from the live owner, and not touched again:
                // the minting await below can span further edits, and
                // re-reading across it would let the request's MODE differ
                // from the one the button was pressed on.
                let current = draft.snapshot();
                // Read the live choice, including a queued submit predating the
                // first registry observation of a newly created sandbox host.
                if unresolved_creation_origin(
                    (!host_fixed)
                        .then(|| {
                            prefill_for_submit
                                .as_ref()
                                .and_then(|source| source.host)
                                .or_else(|| open_host.as_ref().map(|host| host.id))
                        })
                        .flatten(),
                    &hosts,
                    *chosen_host.peek(),
                    current.recipe().as_deref(),
                ) {
                    error.set(Some("The source host's placement is unavailable; wait for Hosts to refresh or choose a target explicitly".into()));
                    return;
                }
                let launch = if current.tab() == LaunchTab::Agent {
                    if let Some(reason) = current.container_refusal() {
                        error.set(Some(reason));
                        return;
                    }
                    if current.harness().is_none() {
                        return;
                    }
                    if current.permissions() == Some(LaunchPermission::Custom)
                        && farhelm_proto::split_permission_flags(&structured_permission_flags())
                            .is_err()
                    {
                        error.set(Some(CUSTOM_PERMISSION_FLAGS_REFUSAL.into()));
                        return;
                    }
                    match wrapper_words(&structured_wrapper()) {
                        Ok(_) => {}
                        Err(reason) => {
                            error.set(Some(reason));
                            return;
                        }
                    }
                    // The flags read was unconditional in the old snapshot
                    // builder, including synchronous guarded-preview submission.
                    let _ = structured_permission_flags();
                    current
                        .launch_intent()
                        .expect("the live harness was checked above")
                } else {
                    // The RAW bytes while untouched, not the escaped display
                    // the field shows — see [`submitted_field`]
                    // (item2-review2.md F5).
                    // Replacing browser required-field validation keeps its
                    // empty-command refusal visible at Launch. Accepted fresh
                    // retries keep their existing exemption from this check.
                    if command_requires_text && current.invocation().is_empty() {
                        error.set(Some("Enter an agent command before launching".to_string()));
                        return;
                    }
                    let Some(launch) = current.launch_intent() else {
                        error.set(Some(
                            "Say whether this command runs in YOLO mode (YOLO) before \
                         launching"
                                .to_string(),
                        ));
                        return;
                    };
                    launch
                };
                if let Some(target) = current.target() {
                    let recipe_name = target.recipe().to_string();
                    if host_fixed {
                        error.set(Some(
                            "Restart and Replace keep the existing environment".into(),
                        ));
                        return;
                    }
                    let catalog = sandbox_catalog.peek();
                    let ready = catalog
                        .as_ref()
                        .and_then(|answer| answer.as_ref().ok())
                        .and_then(|(recipes, settings)| {
                            recipes
                                .iter()
                                .find(|recipe| recipe.name == recipe_name)
                                .map(|recipe| {
                                    crate::environment_ui::ProviderUi(target.provider())
                                        .recipe_refusal(recipe, settings)
                                })
                        });
                    match ready {
                        Some(None) => {}
                        Some(Some(reason)) => {
                            error.set(Some(reason));
                            return;
                        }
                        None => {
                            error.set(Some(
                                "The selected recipe is unavailable; check Settings → Recipes"
                                    .into(),
                            ));
                            return;
                        }
                    }
                    let confirmed = sandbox_consent.peek().as_ref() == Some(&current);
                    sandbox_consent.set(None);
                    let base = base.clone();
                    let snapshot = current.clone();
                    let naming = current.checkout_naming();
                    let name_kind = naming.template_name().map(|(_, kind)| kind).or_else(|| {
                        matches!(naming.source, Some(CheckoutNameSource::Clone(_)))
                            .then_some(farhelm_proto::launcher::TemplateNameKind::Base)
                    });
                    let title = naming
                        .template_name()
                        .map(|(name, _)| name.to_string())
                        .or_else(|| {
                            naming.default_title(
                                current.title_edited(),
                                current.title_raw_seed().as_deref(),
                                current.destination().repo(),
                            )
                        })
                        .unwrap_or_else(|| current.effective_title());
                    let repo = match current.destination() {
                        DestinationDraft::Existing { .. } => None,
                        DestinationDraft::Github { repo } => Some(repo.identifier()),
                        DestinationDraft::GithubInput { .. } => {
                            error.set(Some("Select a GitHub repository before launching".into()));
                            return;
                        }
                    };
                    let SubmitGuards {
                        launch: op_guard,
                        source: unused_source_claim,
                    } = held
                        .take()
                        .expect("this accepted submission owns its token");
                    drop(unused_source_claim);
                    spawn(async move {
                        let skip_billing = !target.provider().asks_before_creating()
                            || api::fetch_tensorlake_settings(&base)
                                .await
                                .is_ok_and(|settings| settings.create_without_asking);
                        if !confirmed && !skip_billing {
                            sandbox_question.set(Some(snapshot));
                            quick_launch_hidden.set(false);
                            drop(op_guard);
                            return;
                        }
                        let key = match mint_intent_key().await {
                            Ok(key) => key,
                            Err(reason) => {
                                error.set(Some(reason));
                                return;
                            }
                        };
                        let conversation = current.conversation();
                        let prompt = current.prompt();
                        let agent = match &launch {
                            LaunchIntent::Structured(selection) => api::CreateAgent::Structured {
                                selection,
                                conversation: (!conversation.is_empty())
                                    .then_some(conversation.as_str()),
                                prompt: prompt.as_deref(),
                            },
                            LaunchIntent::Command(command) => api::CreateAgent::Command(command),
                        };
                        let mut request = api::sandbox_launch_body("~", agent, &title, &key);
                        if let Some(repo) = repo {
                            request["github_checkout"] = serde_json::json!({
                                "repo": repo, "title": api::optional_create_title(&title), "preview": null,
                            });
                            request["cwd"] = serde_json::json!("");
                        }
                        launch_attempted.set(LaunchAttempt::None);
                        release_submitted_composer(mounted_opening, previous_presentation);
                        pending_launch.set(Some(PendingLaunch::new(
                            LaunchAction::Launch,
                            crate::environment_ui::target_label(&target, &[]),
                            Some(title),
                        )));
                        on_create_dispatch.call(());
                        let result = api::create_environment_session(
                            &base,
                            &target,
                            confirmed,
                            name_kind,
                            request,
                            move |host, run| sandbox_run.set(Some((host, run))),
                        )
                        .await;
                        sandbox_run.set(None);
                        on_settled.call(());
                        let never_shown =
                            new_was_never_shown(quick_origin, *quick_launch_hidden.peek());
                        let (press_current, naming_allowed) = if result.is_ok() {
                            completed_launch_input(
                                mounted_opening,
                                opening_generation,
                                press_baseline,
                                launch_activity,
                                never_shown
                                    && preferences.0.peek().name_session_on_quick_launch
                                        != Some(false),
                            )
                            .await
                        } else {
                            (None, false)
                        };
                        let admitted =
                            result.is_ok() || admit_composer_return(mounted_opening, None).await;
                        // Admission protects presentation, not settlement. A
                        // retired opening must not keep its progress pill alive.
                        pending_launch.set(None);
                        if !admitted {
                            on_retired.call(mounted_opening);
                            return;
                        }
                        match result {
                            Ok(session) => {
                                let submitted_launch = match launch {
                                    LaunchIntent::Structured(selection) => Some(selection),
                                    _ => None,
                                };
                                drop(op_guard);
                                COMPOSER_DIALOG
                                    .for_generation("data-opening-generation", mounted_opening)
                                    .release();
                                on_created.call(CreatedSession {
                                    session,
                                    submitted_launch,
                                    new_was_never_shown: never_shown && naming_allowed,
                                    opening: mounted_opening,
                                    press_baseline,
                                    press_current,
                                });
                            }
                            Err(reason) => {
                                quick_launch_hidden.set(false);
                                launch_attempted.set(if reason.is_unknown() {
                                    LaunchAttempt::Unknown
                                } else {
                                    LaunchAttempt::Refused
                                });
                                error.set(Some(reason.to_string()));
                            }
                        }
                    });
                    return;
                }
                // The HOST is derived here too, from the live signal — never
                // from what the last render computed. The same one-turn window
                // the agent has: changing the selector and pressing create in
                // one turn reaches this handler before any re-render, and a
                // captured host would send the create to the PREVIOUS machine
                // while the selector on screen names another.
                let target_now = create_target.peek().clone();
                let selected_now = if restart_mode {
                    prefill_for_submit.as_ref().and_then(|prefill| prefill.host)
                } else {
                    effective_create_host(&hosts, chosen_host.peek().to_owned(), open_host.as_ref())
                };
                if !restart_host_confirmed {
                    error.set(Some("Restart keeps the source session's host, but its installation cannot be confirmed; close and reopen restart with once that host is back".into()));
                    return;
                }
                if !remembered_destination_matches(
                    remembered_destination.peek().as_ref(),
                    live_destination.peek().as_ref(),
                ) {
                    return;
                }
                // A folder nobody chose must be this host's default before it is
                // sent, or a host switch would launch the previous host's folder.
                if !restart_mode && folder_default.peek().pending {
                    let snapshot = draft.snapshot();
                    if !snapshot.folder_is_explicit()
                        && matches!(snapshot.destination(), DestinationDraft::Existing { .. })
                    {
                        return;
                    }
                }
                // The host target must have caught up with the selector before
                // the request can bind its idempotency and connection claims.
                if !target_matches_selection(selected_now, &hosts, target_now.as_ref()) {
                    error.set(Some(
                    "The target host changed while preparing this request, so nothing was sent — check the target and try again"
                        .to_string(),
                ));
                    return;
                }
                if !selected_now.is_some_and(|id| {
                    hosts
                        .iter()
                        .find(|host| host.id == id)
                        .is_some_and(|host| host.available_for_launch(None))
                }) {
                    return;
                }
                // From here this launch is bound to an existing host, through
                // its wake and preview to the create. Suspend and Resume take
                // no page token, so this target is what refuses them for this
                // host (and only this host) while the launch's guard lives.
                if let (Some(host), Some(guards)) = (selected_now, held.as_ref()) {
                    guards.launch.target(host);
                }
                if selected_now
                    .is_some_and(|id| hosts.iter().any(|host| host.id == id && host.suspended))
                    && matches!(
                        draft.snapshot().destination(),
                        DestinationDraft::Github { .. }
                    )
                {
                    let host = selected_now.expect("the suspended host was found");
                    let base = base.clone();
                    let current = draft.snapshot();
                    let Some(authority) =
                        hosts.iter().find(|row| row.id == host).and_then(|host| {
                            checkout_preview_authority(
                                host,
                                &current,
                                observed_checkout_revision(),
                                preview_agent_now(),
                            )
                        })
                    else {
                        return;
                    };
                    let naming = current.checkout_naming();
                    let automatic_name = naming
                        .default_title(
                            current.title_edited(),
                            current.title_raw_seed().as_deref(),
                            current.destination().repo(),
                        )
                        .and(naming.source);
                    let pending = GuardedSubmitStamp::new(
                        rendered_opening,
                        authority,
                        current.prompt(),
                        current.conversation(),
                        replace_guard,
                        press_baseline,
                    );
                    resume_display_handoff.set(None);
                    draft.pin_submitted_host(host);
                    // Local validation owns the source without replacing its
                    // previous outcome. An actual wake starts new work on it.
                    if let Some((source, _)) =
                        held.as_ref().and_then(|guards| guards.source.as_ref())
                    {
                        session_operations.errors.write().remove(source);
                    }
                    release_submitted_composer(mounted_opening, previous_presentation);
                    resuming_host.set(Some(HostResumePreparation {
                        host,
                        name: hosts
                            .iter()
                            .find(|row| row.id == host)
                            .map(HostOption::display_name)
                            .unwrap_or_else(|| "host".into()),
                    }));
                    launch_attempted.set(LaunchAttempt::None);
                    error.set(None);
                    let mut guards = held
                        .take()
                        .expect("this accepted submission owns its token");
                    spawn(async move {
                        let result = api::resume_host_connected(&base, host).await;
                        let handoff = result.as_ref().ok().and_then(|fresh| {
                            completed_resume_display_handoff(
                                target_now.as_ref(),
                                live_resume_display.peek().as_ref(),
                                &super::shared::host_options(fresh),
                            )
                        });
                        resume_display_handoff.set(handoff);
                        let reason = match result {
                            Ok(fresh) => {
                                let connection = super::shared::host_options(&fresh)
                                    .into_iter()
                                    .find(|row| {
                                        row.id == host
                                            && Some(CreateTarget::new(
                                                row.id,
                                                row.incarnation.clone(),
                                            )) == target_now
                                    })
                                    .map(|row| row.connection);
                                if let Some(connection) = connection {
                                    // Successful wake continues hidden through
                                    // preview. The existing slot owns the same
                                    // token until its internal submit takes it.
                                    guarded_submit_pending.set(Some((
                                        pending.after_resume(connection, automatic_name),
                                        Some(guards),
                                    )));
                                    // Starts this press's bounded wait for the
                                    // window to see the connection (the effect
                                    // after `discard_guarded_preparation`).
                                    let next = woken_connection_wait.peek().wrapping_add(1);
                                    woken_connection_wait.set(next);
                                    return;
                                } else {
                                    "The target host changed while resuming; check the target and launch again".into()
                                }
                            }
                            Err(reason) => {
                                format!("Resuming the host failed: {reason}")
                            }
                        };
                        resume_display_handoff.set(None);
                        launch_attempted.set(LaunchAttempt::Refused);
                        settle_prepared_source(
                            &mut guards.source,
                            Some(&reason),
                            session_operations,
                            shared_replace_progress,
                            on_settled,
                        );
                        error.set(Some(reason));
                        return_prepared_composer(
                            mounted_opening,
                            resuming_host,
                            on_retired,
                            guards.launch,
                        )
                        .await;
                    });
                    return;
                }
                // "Replace with" keeps the source's own host (SPEC.md's
                // replace-with bullet; clone is the way to a different
                // host). A `prefill` with no recorded host (a helm old
                // enough to omit `Session::host`) has nothing to compare
                // against and is let through — the helm's own same-host
                // refusal (`ReplaceReq::with`'s doc) is the backstop for
                // that rare case, exactly the way this whole check mirrors
                // that refusal for the ordinary case, so the user sees the
                // words here instead of waiting on a round trip to see
                // them.
                let replace_source = prefill_for_submit
                    .as_ref()
                    .and_then(|prefill| prefill.mode.replace_source())
                    .map(str::to_owned);
                if replace_source.is_some()
                    && prefill_for_submit.as_ref().is_some_and(|prefill| {
                        prefill.host.is_some() && prefill.host != selected_now
                    })
                {
                    // Two ways to get here that deserve different words: the
                    // user picked another host in the selector (say so, and
                    // point at clone), or the reseed effect never bound the
                    // source's host because its install identity no longer
                    // matched (`CloneHostState::Unconfirmable`) and the
                    // selector fell back to the default host — blaming the
                    // user for a host change they never made would be wrong.
                    let unconfirmable =
                        matches!(*clone_host_state.peek(), CloneHostState::Unconfirmable);
                    error.set(Some(
                        if unconfirmable {
                            "Replace with keeps the source session's own host, but that host \
                         could not be confirmed (the source reports a different installation \
                         now), so this create was not sent — replace with cannot proceed \
                         until the source's host is back; clone is the way to start a session \
                         elsewhere"
                        } else {
                            "Replace with keeps the source session's own host, so this create was \
                         not sent — clone is the way to start a session on a different host"
                        }
                        .to_string(),
                    ));
                    return;
                }
                // No host, no create. The helm would default a hostless body
                // to its local row — usually the right answer, and not one
                // this form may reach by omission while its own selector is
                // still blank. Saying so beats creating on a machine the
                // user was never shown.
                let _ = (cwd(), cwd_edited());
                observe_title_fields();
                let current = draft.snapshot();
                let Some(mut binding) = current.request(RequestContext {
                    host: hosts
                        .iter()
                        .find(|host| Some(host.id) == selected_now)
                        .map(|host| (host.id, host.incarnation.as_str())),
                    launch,
                    replace_source: replace_source.as_deref(),
                    checkout: None,
                }) else {
                    error.set(Some(if hosts_loaded {
                        "This helm reported no hosts at all, so this request cannot be sent"
                            .to_string()
                    } else {
                        "The host list has not loaded yet, so this request was not sent — it \
                         would have gone to whichever host the helm picked rather than one \
                         you chose"
                            .to_string()
                    }));
                    return;
                };
                // A guarded-preview effect can call submit synchronously. Keep
                // its old conditional subscriptions while request() alone supplies
                // the projected values used by the ordered validations below.
                if matches!(binding.agent, LaunchIntent::Structured(_)) {
                    let _ = conversation();
                    if binding.conversation.is_some() {
                        let _ = conversation();
                    }
                    let _ = prompt();
                }
                if let Some(message) = &binding.prompt {
                    let refusal = farhelm_proto::launcher::check_initial_prompt(message).err()
                    .map(crate::local_validation_notice)
                    .or_else(|| match &binding.agent {
                        LaunchIntent::Structured(selection) if !selection.harness.supports_initial_prompt(binding.conversation.is_some()) => Some("This agent cannot combine an initial prompt with a named conversation".into()),
                        _ => None,
                    });
                    if let Some(refusal) = refusal {
                        error.set(Some(refusal));
                        return;
                    }
                }
                if binding.conversation.is_some() && {
                    let _ = destination_draft();
                    draft.snapshot().destination().repo().is_some()
                } {
                    error.set(Some("Named conversations cannot start in a new managed checkout; choose an existing folder".into()));
                    return;
                }
                if crate::launch_composer::scoped_query(&composer_search()).0
                    == crate::launch_composer::SearchScope::Github
                {
                    return;
                }
                if let Some(CreatePrefill {
                    mode:
                        LauncherMode::Restart {
                            source,
                            baseline,
                            opened_launch,
                            conversation_required,
                        },
                    ..
                }) = prefill_for_submit.as_ref()
                {
                    let edit = current
                        .restart_edit()
                        .expect("restart has a complete launch");
                    if &edit == baseline {
                        return;
                    }
                    if *conversation_required && current.conversation().is_empty() {
                        error.set(Some("Enter a conversation to restart".into()));
                        return;
                    }
                    if model_open() && !model_draft().is_empty() || model_draft_error().is_some() {
                        error.set(Some("Choose a model before restarting".into()));
                        return;
                    }
                    match &edit {
                        api::RestartEdit::Agent(selection)
                        | api::RestartEdit::NamedAgent(selection, _) => {
                            if !crate::launch_composer::selection_fits_catalog(
                                selection,
                                catalog_for_submit.as_deref(),
                            ) {
                                return;
                            }
                        }
                        api::RestartEdit::Command(command) => {
                            if let Err(reason) = command.validate() {
                                error.set(Some(crate::local_validation_notice(reason)));
                                return;
                            }
                        }
                    }
                    let Some(bridge) = restart_bridge else {
                        return;
                    };
                    let source = source.clone();
                    let opened_launch = opened_launch.clone();
                    let base = base.clone();
                    let bound = binding.clone();
                    let host_name = host_label_for_restart.clone();
                    // The key only names the shared confirmation's draft. Restart
                    // sends no retry key and never enters ambiguous-create recovery.
                    document::eval(
                        "(() => { const dialog = document.querySelector('.create-session-form'); const submit = dialog?.querySelector('.create-session-submit'); (submit && !submit.disabled ? submit : dialog)?.focus({preventScroll:true}); })()",
                    );
                    // An admitted restart is progress. The shared busy refusal
                    // must not become an announced error while its POST or host
                    // preference write is still running.
                    launch_attempted.set(LaunchAttempt::None);
                    error.set(None);
                    let SubmitGuards {
                        launch: op_guard,
                        source: unused_source_claim,
                    } = held
                        .take()
                        .expect("this accepted submission owns its token");
                    drop(unused_source_claim);
                    spawn(async move {
                        let held = intent_key.peek().clone().filter(|(_, held)| held == &bound);
                        let key = match held {
                            Some((key, _)) => key,
                            None => match mint_intent_key().await {
                                Ok(key) => {
                                    intent_key.set(Some((key.clone(), bound.clone())));
                                    key
                                }
                                Err(reason) => {
                                    launch_attempted.set(LaunchAttempt::Refused);
                                    error.set(Some(reason));
                                    return;
                                }
                            },
                        };
                        let allow_yolo = yolo_confirmed_key.peek().as_deref() == Some(key.as_str());
                        if let Some(answered) =
                            stop_asking_for.filter(|answer| answer.key().refused_key == key)
                            && allow_yolo
                            && let Err(reason) =
                                crate::yolo_confirm::stop_asking(&base, bound.host, &host_name)
                                    .await
                        {
                            yolo_confirmed_key.set(None);
                            yolo_error.set(Some(reason));
                            open_yolo_question(key, answered.key().ask.clone());
                            return;
                        }
                        if rendered_opening != *opening_generation.peek() {
                            return;
                        }
                        if create_target.peek().as_ref() != target_now.as_ref() {
                            launch_attempted.set(LaunchAttempt::Refused);
                            error.set(Some("The source host changed while preparing restart; close and reopen restart with".into()));
                            return;
                        }
                        let same_launch = bridge.current.try_read().is_ok_and(|shown| {
                            shown.as_ref().is_some_and(|shown| {
                                shown.launch.as_ref() == opened_launch.as_deref()
                            })
                        });
                        if !same_launch {
                            launch_attempted.set(LaunchAttempt::Refused);
                            error.set(Some(
                            "This session's launch settings changed; close and reopen restart with"
                                .into(),
                        ));
                            return;
                        }
                        if let Err(reason) = bridge.begin(edit.clone()) {
                            launch_attempted.set(LaunchAttempt::Refused);
                            error.set(Some(reason));
                            return;
                        }
                        let pending =
                            PendingLaunch::new(LaunchAction::Restart, host_name.clone(), None);
                        release_submitted_composer(mounted_opening, previous_presentation);
                        pending_launch.set(Some(pending.clone()));
                        let result = api::restart_session(
                            &base,
                            &source,
                            restart_stop_first,
                            Some(&edit),
                            allow_yolo,
                        )
                        .await;
                        // The source claim and detail-read boundary outlive this
                        // dialog. Settle before waiting for another modal; only
                        // the form's own progress/question/error needs admission.
                        if let Err(refusal) = &result
                            && !refusal.outcome.requires_yolo()
                        {
                            bridge.error.call(if refusal.deadline_expired {
                                pending.unknown_outcome(false)
                            } else {
                                refusal.text.clone()
                            });
                        }
                        bridge.finish(result.is_ok());
                        let admitted =
                            result.is_ok() || admit_composer_return(mounted_opening, None).await;
                        pending_launch.set(None);
                        if !admitted {
                            on_retired.call(mounted_opening);
                            return;
                        }
                        match result {
                            Ok(_) => {
                                drop(op_guard);
                                COMPOSER_DIALOG
                                    .for_generation("data-opening-generation", mounted_opening)
                                    .release();
                                on_restarted.call(mounted_opening);
                            }
                            Err(refusal) if refusal.outcome.requires_yolo() => {
                                open_yolo_question(
                                    key,
                                    crate::yolo_confirm::YoloAsk {
                                        host: Some(bound.host),
                                        host_name,
                                        reason: crate::yolo_confirm::YoloReason::of_launch(
                                            match &bound.agent {
                                                LaunchIntent::Structured(selection) => {
                                                    Some(selection)
                                                }
                                                LaunchIntent::Command(_) => None,
                                            },
                                        ),
                                    },
                                );
                            }
                            Err(refusal) => {
                                launch_attempted.set(if refusal.deadline_expired {
                                    LaunchAttempt::Unknown
                                } else {
                                    LaunchAttempt::Refused
                                });
                                error.set(Some(if refusal.deadline_expired {
                                    pending.unknown_outcome(false)
                                } else {
                                    refusal.text
                                }));
                            }
                        }
                    });
                    return;
                }
                let fresh_draft_snapshot = move || {
                    let current = draft.snapshot();
                    (
                        current.destination().repo().cloned(),
                        current.tab(),
                        current.harness(),
                        current.model(),
                        current.effort(),
                        current.permissions(),
                        submitted_field(
                            &current.invocation(),
                            current.invocation_edited(),
                            current.invocation_raw_seed().as_deref(),
                        ),
                        (
                            current.command_yolo(),
                            current.command_agent(),
                            current.command_resume_on(),
                            submitted_field(
                                &current.command_resume(),
                                current.command_resume_edited(),
                                current.command_resume_raw_seed().as_deref(),
                            ),
                        ),
                        title_for_launch(),
                        *chosen_host.peek(),
                        current.prompt(),
                    )
                };
                let fresh_snapshot = fresh_draft_snapshot();
                let mut replaying_fresh = false;
                match draft.snapshot().destination() {
                    DestinationDraft::Existing { .. } => {}
                    DestinationDraft::GithubInput { .. } => {
                        error.set(Some("Select a GitHub repository before launching".into()));
                        return;
                    }
                    DestinationDraft::Github { repo } => {
                        let installation = hosts
                            .iter()
                            .find(|host| host.id == binding.host)
                            .and_then(|host| host.identity.as_deref());
                        let Some(installation) = installation else {
                            error.set(Some(
                            "The host installation is not verified; a checkout cannot be created"
                                .into(),
                        ));
                            return;
                        };
                        let held = github_attempt.peek().clone();
                        if let Some((original, attempt)) = held.filter(|(original, _)| {
                            same_fresh_intent(original, &binding, &repo, installation)
                        }) {
                            binding = original;
                            intent_key.set(Some((attempt.key, binding.clone())));
                            replaying_fresh = true;
                        } else if let PreviewState::Ready { authority, preview } =
                            preview_state.peek().clone()
                            && live_preview_authority
                                .peek()
                                .as_ref()
                                .is_some_and(|live| authority.accepts(live, &preview))
                            && preview.installation_identity.raw() == installation
                            && preview.host == binding.host.to_string()
                            && Some(preview.incarnation) == connection_claim(&hosts, binding.host)
                            && authority.title.as_deref() == Some(binding.title.as_str())
                            && authority.agent == preview_agent_now()
                            && *preview_revision.peek() == *applied_preview_revision.peek()
                        {
                            // No await separates the preliminary binding and this
                            // admitted preview: reuse its draft snapshot and frozen
                            // launch rather than reading a different checkout title.
                            binding = current
                                .request(RequestContext {
                                    host: Some((binding.host, &binding.incarnation)),
                                    launch: binding.agent.clone(),
                                    replace_source: binding.replace_source.as_deref(),
                                    checkout: Some(CheckoutInput::Preview {
                                        repo: &repo,
                                        preview: &preview,
                                    }),
                                })
                                .expect("the preliminary binding already resolved its host");
                        } else {
                            error.set(Some(
                                "Wait for a current checkout preview before launching".into(),
                            ));
                            return;
                        }
                    }
                }
                let fresh_authority = live_preview_authority.peek().clone();
                // An accepted request replays its recorded launch snapshot.
                // New requests still validate against today's catalog.
                if !replaying_fresh
                    && let LaunchIntent::Structured(selection) = &binding.agent
                    && !crate::launch_composer::selection_fits_catalog(
                        selection,
                        catalog_for_submit.as_deref(),
                    )
                {
                    return;
                }
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
                // The name the YOLO confirmation and the pending-launch status
                // show for this create's host, from the same snapshot. Phase-free:
                // a suspended host is resumed before the create is sent.
                let yolo_host_name = hosts
                    .iter()
                    .find(|host| host.id == binding.host)
                    .map(HostOption::display_name)
                    .unwrap_or_else(|| "This host".to_string());
                // An admitted attempt is progress, not a refusal. Do not show
                // the busy prerequisite merely because its request is running.
                launch_attempted.set(LaunchAttempt::None);
                error.set(None);
                folder_refusal.set(None);
                let catalog_for_recheck = catalog_for_submit.clone();
                let hosts = hosts.clone();
                let SubmitGuards {
                    launch: op_guard,
                    source: session_claim,
                } = held
                    .take()
                    .expect("this accepted submission owns its token");
                spawn(async move {
                    let mut source_claim = session_claim;
                    let mut held = Some(op_guard);
                    // Preparation failures return to one epilogue. It releases
                    // the source claim immediately, but a hidden draft retains
                    // the launch token until its error/question can be shown.
                    async {
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
                            // Suspension deliberately invalidates the connection token.
                            // Resume first, then bind both minting and the create to the
                            // new connection, preserving the selected installation.
                            let (_hosts, expected_incarnation, created_host_identity) = if hosts
                                .iter()
                                .any(|host| host.id == binding.host && host.suspended)
                            {
                                // Both failures below happen before any create is sent, so
                                // the launch is Refused, never Unknown: whatever the
                                // resume did, no session can have started from it.
                                resume_display_handoff.set(None);
                                draft.pin_submitted_host(binding.host);
                                if let Some((source, _)) = source_claim.as_ref() {
                                    session_operations.errors.write().remove(source);
                                }
                                release_submitted_composer(mounted_opening, previous_presentation);
                                resuming_host.set(Some(HostResumePreparation {
                                    host: binding.host,
                                    name: yolo_host_name.clone(),
                                }));
                                let result = api::resume_host_connected(&base, binding.host).await;
                                let fresh = match result {
                                    Ok(hosts) => super::shared::host_options(&hosts),
                                    Err(reason) => {
                                        resume_display_handoff.set(None);
                                        launch_attempted.set(LaunchAttempt::Refused);
                                        error.set(Some(format!("Resuming the host failed: {reason}")));
                                        return;
                                    }
                                };
                                let current = fresh.iter().find(|host| host.id == binding.host);
                                if current.map(|host| &host.incarnation) != Some(&binding.incarnation) {
                                    resume_display_handoff.set(None);
                                    launch_attempted.set(LaunchAttempt::Refused);
                                    error.set(Some("The host installation changed while resuming; review the target and launch again".into()));
                                    return;
                                }
                                resume_display_handoff.set(completed_resume_display_handoff(
                                    target_now.as_ref(),
                                    live_resume_display.peek().as_ref(),
                                    &fresh,
                                ));
                                let connection = connection_claim(&fresh, binding.host);
                                let identity = current.map(|host| host.identity.clone());
                                (fresh, connection, identity)
                            } else {
                                (hosts, expected_incarnation, created_host_identity)
                            };
                            let mut attempts = 0;
                            let (key, bound) = loop {
                                let held = intent_key.peek().clone();
                                if let Some((key, held_binding)) = held
                                    && held_binding == binding
                                {
                                    break (key, held_binding);
                                }
                                if attempts >= MINT_ATTEMPTS {
                                    launch_attempted.set(LaunchAttempt::Refused);
                                    error.set(Some(
                                        "The form kept changing while an idempotency key was being \
                                         generated, so this create was not sent; try again"
                                            .to_string(),
                                    ));
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
                                        launch_attempted.set(LaunchAttempt::Refused);
                                        error.set(Some(format!(
                                            "Could not generate an idempotency key for this create, so \
                                             it was not sent (a retry could otherwise create a second \
                                             session): {reason}"
                                        )));
                                        return;
                                    }
                                }
                                // What the form's TEXT says now. Identical on the
                                // ordinary path; different exactly when a queued edit
                                // landed during the mint.
                                //
                                // A catalog refresh is not a new user choice, so it
                                // is frozen at the press. Structured controls are
                                // different. Every
                                // visible harness/model/effort/permission click is a
                                // deliberate edit, and it may have been queued ahead
                                // of the render that disables controls. Re-read that
                                // complete declarative intent so the next key binds
                                // exactly what the person now sees.
                                let launch = if matches!(binding.agent, LaunchIntent::Structured(_)) {
                                    let current = draft.snapshot();
                                    if let Some(reason) = current.container_refusal() {
                                        launch_attempted.set(LaunchAttempt::Refused);
                                        error.set(Some(reason));
                                        return;
                                    }
                                    if current.harness().is_none() {
                                        launch_attempted.set(LaunchAttempt::Refused);
                                        error.set(Some(
                                            "The structured launch choice changed while its idempotency key was being generated; choose a harness and press Launch again".to_string(),
                                        ));
                                        return;
                                    }
                                    match wrapper_words(&structured_wrapper()) {
                                        Ok(_) => {}
                                        Err(reason) => {
                                            launch_attempted.set(LaunchAttempt::Refused);
                                            error.set(Some(reason));
                                            return;
                                        }
                                    }
                                    let _ = structured_permission_flags();
                                    let selection = current
                                        .structured_selection()
                                        .expect("the live harness was checked above");
                                    if current.tab() != LaunchTab::Agent
                                        || !crate::launch_composer::selection_fits_catalog(
                                            &selection,
                                            catalog_for_recheck.as_deref(),
                                        )
                                    {
                                        launch_attempted.set(LaunchAttempt::Refused);
                                        error.set(Some(
                                            "The structured launch choice changed while its idempotency key was being generated; review it and press Launch again".to_string(),
                                        ));
                                        return;
                                    }
                                    LaunchIntent::Structured(selection)
                                } else {
                                    binding.agent.clone()
                                };
                                let current = draft.snapshot();
                                observe_title_fields();
                                // A managed path/checkout stays the accepted snapshot.
                                // The binding's host facts also stay frozen, including an
                                // older retained attempt's incarnation after reconnect.
                                let checkout =
                                    binding
                                        .github_checkout
                                        .as_ref()
                                        .map(|request| CheckoutInput::Accepted {
                                            cwd: &binding.cwd,
                                            request,
                                        });
                                binding = current
                                    .request(RequestContext {
                                        host: Some((binding.host, &binding.incarnation)),
                                        launch,
                                        replace_source: binding.replace_source.as_deref(),
                                        checkout,
                                    })
                                    .expect("an accepted binding retains its host");
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
                                launch_attempted.set(LaunchAttempt::Refused);
                                error.set(Some(REMEMBERED_DESTINATION_CHANGED.to_string()));
                                intent_key.set(None);
                                return;
                            }
                            if bound.github_checkout.is_some()
                                && (fresh_draft_snapshot() != fresh_snapshot
                                    || live_destination.peek().as_ref() != target_now.as_ref()
                                    || (!replaying_fresh && *live_preview_authority.peek() != fresh_authority))
                            {
                                launch_attempted.set(LaunchAttempt::Refused);
                                error.set(Some("The checkout draft changed while preparing the request; review the preview and press Launch again".into()));
                                return;
                            }
                            // Only a confirmation given for THIS key counts; see
                            // `yolo_confirmed_key`.
                            let allow_yolo = yolo_confirmed_key.peek().as_deref() == Some(key.as_str());
                            // "Don't ask again": mark the host safe before anything
                            // is sent, and send nothing if that fails. The override
                            // is withdrawn on failure too, so a later plain Launch
                            // asks again rather than riding on this confirmation.
                            if let Some(answered) =
                                stop_asking_for.filter(|answered| answered.key().refused_key == key)
                                && allow_yolo
                                && let Err(reason) =
                                    crate::yolo_confirm::stop_asking(&base, bound.host, &yolo_host_name).await
                            {
                                // The answer took the question down; it comes back,
                                // as a new opening, with the reason, so the user can
                                // pick another answer.
                                yolo_confirmed_key.set(None);
                                yolo_error.set(Some(reason));
                                let LauncherYoloQuestion {
                                    refused_key, ask, ..
                                } = answered.key().clone();
                                open_yolo_question(refused_key, ask);
                                return;
                            }
                            let agent = match &bound.agent {
                                LaunchIntent::Command(command) => CreateAgent::Command(command),
                                LaunchIntent::Structured(selection) => CreateAgent::Structured {
                                    selection,
                                    conversation: bound.conversation.as_deref(),
                                    prompt: bound.prompt.as_deref(),
                                },
                            };
                            // The one branch point between the two verbs this form
                            // shares: an ordinary create and a "replace with" send
                            // the SAME body fields (cwd, agent, title, intent key,
                            // host, expected incarnation — see `create_body`, the
                            // single builder both calls share), differing only in
                            // which endpoint receives them and in the source id a
                            // replace-with also names. Everything above this point
                            // — resolution, guards, key minting — has already run
                            // identically for both; only the network call itself
                            // forks.
                            let pending = PendingLaunch::new(
                                if bound.replace_source.is_some() {
                                    LaunchAction::Replace
                                } else {
                                    LaunchAction::Launch
                                },
                                yolo_host_name.clone(),
                                Some(bound.title.clone()),
                            );
                            draft.pin_submitted_host(bound.host);
                            if let Some(source) = &bound.replace_source {
                                session_operations.errors.write().remove(source);
                            }
                            release_submitted_composer(mounted_opening, previous_presentation);
                            pending_launch.set(Some(pending.clone()));
                            // Publish the create wait before ending wake preparation, so
                            // no render can restore modality between these owned steps.
                            resuming_host.set(None);
                            // Owned by a guard, as plain Replace's is: if the
                            // launcher unmounts mid-request, this task is dropped
                            // at its await and the guard still clears "Replacing…"
                            // from the source's row and header.
                            let replace_progress = bound.replace_source.as_ref().map(|source| {
                                crate::replace::ReplaceProgressGuard::start(
                                    shared_replace_progress,
                                    source.clone(),
                                    pending.clone(),
                                )
                            });
                            let create_result = if let Some(checkout) = &bound.github_checkout {
                                let attempt = github_attempt
                                    .peek()
                                    .as_ref()
                                    .filter(|(original, attempt)| original == &bound && attempt.key == key)
                                    .map(|(_, attempt)| attempt.clone())
                                    .unwrap_or_else(|| {
                                        GithubAttempt::new(
                                            key.clone(),
                                            api::fresh_create_body(agent, &key, bound.host, checkout),
                                            checkout.preview.installation_identity.raw().to_owned(),
                                        )
                                    });
                                let mut attempt = attempt;
                                if allow_yolo {
                                    api::confirm_yolo(&mut attempt.body);
                                }
                                // Publish before dispatch. A lost response must leave
                                // the exact payload available to the next explicit retry.
                                github_attempt.set(Some((bound.clone(), attempt.clone())));
                                // The body is the retained original; the source
                                // delete's precondition is the one this submit was
                                // given: the warning the press or answer it acts for saw
                                // (see `submit_fresh_create` and `GuardedSubmitStamp`).
                                if bound.replace_source.is_none() {
                                    on_create_dispatch.call(());
                                }
                                match api::submit_fresh_create(
                                    &base,
                                    bound.replace_source.as_deref(),
                                    replace_guard,
                                    &attempt.body,
                                )
                                .await
                                {
                                    Ok((session, notice)) => {
                                        github_attempt.set(None);
                                        delete_notice.publish(notice);
                                        Ok(session)
                                    }
                                    Err(crate::github_checkout::FreshCreateError::YoloConfirmation(text)) => {
                                        // Refused before anything was dispatched. The
                                        // attempt and its key stay, so the confirmed
                                        // retry is the same request with the override.
                                        github_attempt.set(Some((bound.clone(), attempt)));
                                        Err(api::CreateRefusal {
                                            outcome: api::RefusalOutcome::NeedsYoloConfirmation,
                                            text,
                                            deadline_expired: false,
                                        })
                                    }
                                    Err(failure) => {
                                        let retired = attempt.may_retire_after(&failure);
                                        if retired {
                                            github_attempt.set(None);
                                            intent_key.set(None);
                                            preview_revision.with_mut(|revision| {
                                                *revision =
                                                    revision.checked_add(1).expect("preview revision exhausted")
                                            });
                                        } else {
                                            github_attempt.set(Some((bound.clone(), attempt)));
                                        }
                                        let deadline_expired = matches!(
                                            &failure,
                                            crate::github_checkout::FreshCreateError::DeadlineExpired(_)
                                        );
                                        let mut refusal = api::CreateRefusal::from(format!(
                                            "{}; {}",
                                            failure.message(),
                                            if retired {
                                                "nothing was accepted; review the refreshed preview and press Launch again"
                                            } else {
                                                "the original request is retained; retry reconciles that request without allocating a different checkout"
                                            }
                                        ));
                                        refusal.deadline_expired = deadline_expired;
                                        Err(refusal)
                                    }
                                }
                            } else {
                                match &bound.replace_source {
                                    Some(source) => api::replace_session_with(
                                        &base,
                                        source,
                                        replace_guard,
                                        &bound.cwd,
                                        agent,
                                        &bound.title,
                                        &key,
                                        Some(bound.host),
                                        expected_incarnation,
                                        allow_yolo,
                                    )
                                    .await
                                    .map(|(session, notice)| {
                                        delete_notice.publish(notice);
                                        session
                                    }),
                                    None => {
                                        on_create_dispatch.call(());
                                        create_session(
                                            &base,
                                            &bound.cwd,
                                            agent,
                                            &bound.title,
                                            &key,
                                            Some(bound.host),
                                            expected_incarnation,
                                            allow_yolo,
                                        )
                                        .await
                                    }
                                }
                            };
                            // Shared source progress/errors must settle independently of
                            // Return admission. Only the local pill, draft and launch
                            // token wait behind another modal; the source is usable now.
                            // No await between this drop and `on_settled` below, the
                            // same handoff plain Replace keeps: a fleet read must not
                            // retire the selected source in between.
                            drop(replace_progress);
                            if let Some(source) = &bound.replace_source {
                                if let Err(refusal) = &create_result
                                    && !refusal.outcome.requires_yolo()
                                {
                                    let reason = if refusal.deadline_expired {
                                        pending.unknown_outcome(false)
                                    } else {
                                        refusal.text.clone()
                                    };
                                    session_operations
                                        .errors
                                        .write()
                                        .insert(source.clone(), format!("Replace: {reason}"));
                                } else {
                                    session_operations.errors.write().remove(source);
                                }
                            }
                            drop(source_claim.take());
                            on_settled.call(());
                            let never_shown = new_was_never_shown(quick_origin, *quick_launch_hidden.peek());
                            // Replace follows its still-selected source, not
                            // New/Clone's input rule. Keep that handoff in this
                            // reply turn: its requested refresh may already
                            // prove the removed source absent after any await.
                            let (press_current, naming_allowed) = if create_result.is_ok() && bound.replace_source.is_none() {
                                completed_launch_input(
                                    mounted_opening, opening_generation, press_baseline, launch_activity,
                                    never_shown && preferences.0.peek().name_session_on_quick_launch != Some(false),
                                ).await
                            } else {
                                (None, false)
                            };
                            let admitted = create_result.is_ok() || admit_composer_return(mounted_opening, None).await;
                            pending_launch.set(None);
                            if !admitted {
                                on_retired.call(mounted_opening);
                                return;
                            }
                            match create_result {
                                Ok(session) => {
                                    // The callback may open quick-launch naming, which
                                    // checks this token synchronously. Release before
                                    // that handoff, rather than at the future's end.
                                    drop(held.take());
                                    let submitted_launch = match &bound.agent {
                                        LaunchIntent::Structured(selection) => Some(selection.clone()),
                                        LaunchIntent::Command(_) => None,
                                    };
                                    COMPOSER_DIALOG.for_generation("data-opening-generation", mounted_opening).release();
                                    on_created.call(CreatedSession {
                                        session: enrich_created_session(
                                            session,
                                            bound.host,
                                            created_host_identity,
                                        ),
                                        submitted_launch,
                                        new_was_never_shown: never_shown && naming_allowed,
                                        opening: mounted_opening,
                                        press_baseline,
                                        press_current,
                                    });
                                }
                                Err(e) => {
                                    // Every unsuccessful quick request ends hidden
                                    // preparation, even if its old target no longer owns
                                    // the refusal. Keep that refusal fenced below.
                                    quick_launch_hidden.set(false);
                                    // Marked whatever the form now shows: the key stays
                                    // bound to this setup even when the refusal text is
                                    // fenced off below, and a return to the setup reuses
                                    // it, so its YOLO question must still know.
                                    if !e.outcome.requires_yolo() && !e.outcome.retires_retry() {
                                        uncertain_key.set(Some(key.clone()));
                                    }
                                    // Gated on the target this request was DISPATCHED
                                    // for still being the one on screen: a refusal
                                    // naming host A must not land under a form that
                                    // has since been re-pointed at host B, where it
                                    // would describe a machine the user is not looking
                                    // at and may not even be true.
                                    if create_target.peek().as_ref() == target_now.as_ref() {
                                        let api::CreateRefusal {
                                            outcome,
                                            text: prose,
                                            deadline_expired,
                                        } = e;
                                        if outcome.requires_yolo() {
                                            // The key deliberately survives: this
                                            // attempt was refused before dispatch, but
                                            // an EARLIER attempt under the same key may
                                            // have been accepted with its reply lost,
                                            // and the confirmed retry must still
                                            // reconcile with it rather than start a
                                            // second session. The loud confirmation
                                            // replaces the ordinary error line.
                                            open_yolo_question(
                                                key.clone(),
                                                crate::yolo_confirm::YoloAsk {
                                                    host: Some(bound.host),
                                                    host_name: yolo_host_name.clone(),
                                                    reason: crate::yolo_confirm::YoloReason::of_launch(
                                                        match &bound.agent {
                                                            LaunchIntent::Structured(selection) => {
                                                                Some(selection)
                                                            }
                                                            LaunchIntent::Command(_) => None,
                                                        },
                                                    ),
                                                },
                                            );
                                            drop(held.take());
                                            return;
                                        }
                                        if outcome.retires_retry() {
                                            // A settled refusal or changed target incarnation
                                            // needs a new attempt. Unknown outcomes keep the
                                            // old key so a retry cannot create a second session.
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
                                        if let (Some(target), Some(problem)) =
                                            (target_now.clone(), FolderProblem::recognize(&prose))
                                        {
                                            folder_refusal.set(Some(FolderRefusal {
                                                target,
                                                connection: expected_incarnation,
                                                cwd: bound.cwd.clone(),
                                                host: yolo_host_name.clone(),
                                                problem,
                                            }));
                                        }
                                        // Dispatch cleared the attempt mark so the busy
                                        // prerequisite would not show while the request
                                        // ran; a refusal is a finished attempt again, and
                                        // only a genuine refusal is labelled "Not launched.".
                                        launch_attempted.set(if deadline_expired {
                                            LaunchAttempt::Unknown
                                        } else {
                                            LaunchAttempt::Refused
                                        });
                                        error.set(Some(if deadline_expired {
                                            pending.unknown_outcome(true)
                                        } else {
                                            prose
                                        }));
                                    } else {
                                        // The old request still ended unsuccessfully.
                                        // Say so without leaking its host-specific prose
                                        // or retiring the new target's retry identity.
                                        launch_attempted.set(if e.deadline_expired {
                                            LaunchAttempt::Unknown
                                        } else {
                                            LaunchAttempt::Refused
                                        });
                                        error.set(Some(if e.deadline_expired {
                                            "A launch for an earlier setup timed out; its outcome is unknown"
                                                .into()
                                        } else {
                                            "A launch for an earlier setup was refused".into()
                                        }));
                                    }
                                    drop(held.take());
                                }
                            }
                        }.await;
                    // A failed wake or preparation never dispatched a create,
                    // but its source claim is no longer pending. Keep only the
                    // local token and hidden presentation through the owed Return.
                    // A mint/validation refusal before any wake/create is local
                    // to the draft, so it cannot replace an earlier row outcome.
                    let preparation_reason = resuming_host
                        .peek()
                        .is_some()
                        .then(|| error.peek().clone())
                        .flatten();
                    settle_prepared_source(
                        &mut source_claim,
                        preparation_reason.as_deref(),
                        session_operations,
                        shared_replace_progress,
                        on_settled,
                    );
                    if resuming_host.peek().is_some() {
                        resume_display_handoff.set(None);
                        if let Some(op_guard) = held.take() {
                            return_prepared_composer(
                                mounted_opening,
                                resuming_host,
                                on_retired,
                                op_guard,
                            )
                            .await;
                        }
                    }
                });
            })();
            if let Some(mut guards) = held {
                if continued {
                    // A continuation after a wake can be refused with no error
                    // at all (the host stopped being available for launch, say);
                    // the launcher's own prerequisite text is then the
                    // explanation. A hidden quick launch must reveal
                    // that here, before Return admits it as modal: the reveal
                    // effect reads the token with `peek`, so it saw this
                    // submission as busy, and the token's release is not a
                    // change it observes. Without this, an invisible modal
                    // launcher sat over the page. The opening check mirrors the
                    // validation closure's first one: the only caller passes
                    // the live opening today, and a submission must never
                    // reveal an opening that is not its own.
                    if rendered_opening == *opening_generation.peek() {
                        quick_launch_hidden.set(false);
                    }
                    let reason = error.peek().clone();
                    settle_prepared_source(
                        &mut guards.source,
                        reason.as_deref(),
                        session_operations,
                        shared_replace_progress,
                        on_settled,
                    );
                    spawn(return_prepared_composer(
                        mounted_opening,
                        resuming_host,
                        on_retired,
                        guards.launch,
                    ));
                } else {
                    drop(guards);
                }
            }
        }
    });
    // Capture the ordered page count in the user's original Rust handler;
    // internal preview reentry instead passes the count its stamp already owns.
    let submit_answer = Callback::new(move |(guard, stop_first, opening)| {
        submit_composer.call((guard, stop_first, opening, launch_activity.snapshot(), None));
    });
    // Enter on a choice (a tab, harness, effort or permission segment, the
    // closed host picker, a command-mode control) applies that choice and
    // launches. In Replace with a launch is the source delete's
    // confirmation, so it submits with this render's `replace_guard`, the
    // precondition of the warning on screen at the key press, never one a
    // later render computes (SPEC.md "Replace with").
    //
    // It calls `submit_composer` directly, in the same handler that applied
    // the choice. It used to go through the browser (`requestSubmit`), whose
    // `onsubmit` landed as a later event: on desktop after a JavaScript
    // round trip that let renders, and their precondition, move on between
    // the key press and the submit. No render is needed in between: on web
    // that `onsubmit` already ran nested in this handler, before any render;
    // `submit_composer` reads the live draft and host, and refuses a checkout
    // preview the choice made stale (`preview_revision`, preview agent); and
    // the form is `novalidate`, so the browser validated nothing on the way.
    let launch_from_choice = EventHandler::new(move |_| submit_answer.call(submit_consent));
    let host_select = rsx! {
        crate::environment_ui::PlacementPicker {
            target: environment_target(),
            last_container_host: preferences.0.read().last_container_host,
            places: environment_places.clone(),
            builds: build_status.read().as_ref().and_then(|answer|answer.as_ref().ok()).cloned().unwrap_or_default(),
            recipes: sandbox_answer.as_ref().and_then(|answer| answer.as_ref().ok()).map(|(recipes,_)| recipes.clone()).unwrap_or_default(),
            settings: sandbox_answer.as_ref().and_then(|answer| answer.as_ref().ok()).map(|(_,settings)| settings.clone()),
            disabled: busy || host_fixed,
            on_target: move |target| { if draft.choose_target(target) { folder_open.set(false); folder_active.set(None); template_panel.set(None); } },
        div { class: "launch-composer-row",
            span { class: "launch-composer-row-label", "Host" }
            super::host_picker::HostPicker {
                hosts: hosts.clone(), selected, busy,
                on_recipe: move |_| {},
                on_pick: choose_host, on_enter: launch_from_choice,
            }
        }
        }
    };
    // A visible guarded answer can be discarded directly. A wake continuation
    // owns a hidden draft and token, so cancellation/failure owes Return before
    // releasing either. Take the pair once rather than clearing a guard-bearing
    // slot and allowing a new launch to replace its still-unreported refusal.
    let discard_guarded_preparation = use_callback(move |reason: Option<String>| {
        let pending = guarded_submit_pending.write().take();
        let Some((pending, guard)) = pending else {
            return;
        };
        if let Some(mut guards) = guard {
            launch_attempted.set(LaunchAttempt::Refused);
            let reason = reason.unwrap_or_else(|| {
                "The launch setup changed while the host was waking; review it and launch again"
                    .into()
            });
            settle_prepared_source(
                &mut guards.source,
                Some(&reason),
                session_operations,
                shared_replace_progress,
                on_settled,
            );
            error.set(Some(reason));
            resume_display_handoff.set(None);
            spawn(return_prepared_composer(
                pending.opening,
                resuming_host,
                on_retired,
                guards.launch,
            ));
        } else if let Some(reason) = reason {
            launch_attempted.set(LaunchAttempt::Refused);
            error.set(Some(reason));
        }
    });
    // One bounded wait per press a successful wake parked (SPEC.md "Creation").
    // When it runs out, the press is discarded only if it is still that press
    // and the window still has not seen the woken host connect. A press that
    // saw the connection and now waits for its checkout preview is never cut
    // off, and a connection that shows up after the discard finds nothing to
    // submit. The discard returns the launcher with its draft and releases the
    // token, which admits New, quick launches and host changes again.
    use_effect(move || {
        let wait = woken_connection_wait();
        if wait == 0 {
            return;
        }
        spawn(async move {
            crate::reader::sleep_ms(WOKEN_CONNECTION_WAIT_MS).await;
            if *woken_connection_wait.peek() != wait {
                return;
            }
            let expired = parked_press_awaiting_woken_connection(
                guarded_submit_pending
                    .peek()
                    .as_ref()
                    .map(|(pending, guard)| (pending, guard.is_some())),
                live_preview_authority.peek().as_ref(),
                live_resume_display.peek().as_ref(),
            );
            if expired {
                discard_guarded_preparation.call(Some(WOKEN_CONNECTION_TIMEOUT.into()));
            }
        });
    });
    // An unchanged accepted answer is spent before ordinary submit. Only a
    // resumed opening retains a claim here; its next step transfers that same
    // owner, while a visible guarded answer claims normally at dispatch.
    use_effect(move || {
        let pending = guarded_submit_pending
            .read()
            .as_ref()
            .map(|(pending, guard)| (pending.clone(), guard.is_some()));
        let Some((pending, retained_guard)) = pending else {
            return;
        };
        let answered_guard = live_preview_authority().and_then(|mut authority| {
            authority.agent = preview_agent_now();
            pending.guard_if_current(
                opening_generation(),
                authority,
                &prompt(),
                &conversation(),
                draft.snapshot().checkout_naming().source.as_ref(),
            )
        });
        let Some(answered_guard) = answered_guard else {
            discard_guarded_preparation.call(None);
            return;
        };
        // Still the old connection, or a fresh token already visible in
        // Connecting: even a Ready preview must wait until this form observes
        // the host as connected. The bounded wait above ends this.
        if pending.awaiting_woken_connection(
            live_preview_authority().as_ref(),
            live_resume_display().as_ref(),
        ) {
            return;
        }
        let busy = ops.busy();
        if !retained_guard && (busy || ops.busy_now()) {
            return;
        }
        let _ = destination_draft();
        let DestinationDraft::Github { .. } = draft.snapshot().destination() else {
            discard_guarded_preparation.call(None);
            return;
        };
        match preview_state() {
            PreviewState::Pending => {}
            PreviewState::Ready { authority, preview }
                if live_preview_authority
                    .peek()
                    .as_ref()
                    .is_some_and(|live| authority.accepts(live, &preview)) =>
            {
                let continuing = guarded_submit_pending.write().take();
                if let Some((pending, guard)) = continuing {
                    submit_composer.call((
                        answered_guard,
                        false,
                        pending.opening,
                        pending.press_baseline,
                        guard,
                    ));
                }
            }
            PreviewState::Failed { message, .. } => {
                discard_guarded_preparation.call(Some(message));
            }
            PreviewState::Ready { .. } => {
                discard_guarded_preparation.call(Some(
                    "Wait for a current checkout preview before launching".into(),
                ));
            }
        }
    });
    // A scope-owned callback allocated during every render would accumulate
    // until this launcher closes. Keep one handle while still reading hidden
    // state at the actual mount boundary, after trap setup is submitted.
    let composer_hidden_now = use_callback(move |_| composer_hidden());
    // Body recovery and submitted-return restoration must choose the same
    // target, whichever deferred frame runs first. Initial Restart stays on
    // Cancel; an attempted refusal recovers to its primary action instead.
    let attempted_restart = restart_mode && launch_attempted().was_attempted();
    let loading_setup =
        pending_initial_action().is_some() || !pending_initial_templates.read().is_empty();
    // Diagnostic only: tells a browser test when a parked press has started
    // its bounded wait for the woken connection, before it moves the clock.
    let awaiting_woken_connection = parked_press_awaiting_woken_connection(
        guarded_submit_pending
            .read()
            .as_ref()
            .map(|(pending, guard)| (pending, guard.is_some())),
        live_preview_authority.read().as_ref(),
        live_resume_display.read().as_ref(),
    );
    let submit_label = if restart_mode {
        if restart_stop_first {
            "Stop and restart"
        } else {
            "Restart"
        }
    } else if is_replace_with {
        "Replace"
    } else {
        "Launch"
    };
    // Keep the full target in the accessible action name even when its adjacent
    // summary truncates visually. Command never names a dormant agent draft.
    let target_summary = format!(
        "{}{}{} · {}",
        if *launch_tab.read() == LaunchTab::Command {
            "Command · "
        } else {
            ""
        },
        launch_harness
            .as_ref()
            .map(|harness| format!("{harness} · "))
            .unwrap_or_default(),
        selected_host_label,
        summary_folder,
    );
    let footer_error = error.read().clone().or_else(|| {
        launch_attempted()
            .was_attempted()
            .then_some(launch_refusal)
            .flatten()
            .map(str::to_string)
    });
    let show_footer_notices = footer_error.is_some()
        || (restart_mode && restart_stop_first)
        || (is_replace_with && replace_warning.is_some())
        || (yolo_refusal
            .current()
            .filter(|_| !composer_hidden())
            .is_some_and(|question| {
                intent_key
                    .read()
                    .as_ref()
                    .is_some_and(|(key, _)| *key == question.key().refused_key)
            }));
    rsx! {
        div {
            class: if submitted_wait() { "launch-composer-backdrop launch-request-pending" } else if quick_launch_hidden() { "launch-composer-backdrop quick-launch-pending" } else { "launch-composer-backdrop" },
            role: "presentation",
            onclick: move |_| {
                // This is a handler-time decision. The rendered `busy` value
                // can be one turn stale immediately after submit, when
                // unmounting would cancel the create future before it can
                // reconcile the reply or run its success callback.
                // Restart's editor keeps its draft on backdrop clicks. The
                // same modal trap recovers sequential focus from the backdrop.
                if !restart_mode && !composer_hidden() && !ops.busy_now() && !template_saving() {
                    on_cancel.call(());
                }
            },
            if composer_hidden() {
                if let Some(preparation) = resuming_host() {
                    div { class: "launch-progress-pill", "data-modal-exempt": "",
                        HostResumeProgress { preparation }
                    }
                } else if let Some(pending) = pending_launch() {
                    div { class: "launch-progress-pill", "data-modal-exempt": "",
                        LaunchProgress { pending }
                    }
                }
            }
        ModalDialog {
            handle: COMPOSER_DIALOG,
            root: DialogRoot::Form,
            focus: DialogFocus {
                initial_focus: InitialFocus::Surface {
                    target: if restart_mode { FocusTarget::new(".launch-composer-cancel", None, false) } else { composer_surface_target() },
                    // Sample on mount, after trap setup is submitted. Hidden
                    // preparation isolates without taking a field's focus.
                    hidden_now: composer_hidden_now,
                },
                escape_target: Some(COMPOSER_ESCAPE_TARGET.into()),
                // Loading has no Launch button: the shared container Enter
                // action must resolve the live control rather than submit early.
                main_action: Some(".create-session-submit".into()),
                gap: true,
            },
            class: "create-session-form",
            "data-launcher-mode": if restart_mode { "restart" } else if is_replace_with { "replace" } else { "create" },
            "data-permission-escalation": permission_escalation.read().as_ref().map(|notice| if notice.generation % 2 == 0 { "even" } else { "odd" }),
            tabindex: "-1",
            hidden: composer_hidden(),
            "data-opening-generation": "{mounted_opening}",
            "data-create-target-fingerprint": create_target.read().as_ref().map(|target| target.fingerprint.clone()).unwrap_or_default(),
            // These are diagnostic state, not an authority channel. Keeping
            // the live connection and activation count on the mounted form
            // gives browser tests a synchronous observation point for races
            // that deliberately keep the visible result unchanged.
            "data-initial-templates-ready": launch_templates.read().is_some(),
            // A held place answer needs an observable completion boundary for
            // the other template inputs before a browser can assert it still waits.
            "data-template-catalogs-ready": hosts_loaded && launch_templates.read().is_some()
                && launch_catalog.read().is_some() && sandbox_catalog.read().is_some(),
            "data-history-fetched-revision": "{fetched_history_revision}",
            "data-environment-history-fetched-revision": "{fetched_environment_history_revision}",
            "data-composer-mode": if *launch_tab.read() == LaunchTab::Agent { "structured" } else { "command" },
            "data-browse-generation": "{browse_generation()}",
            "data-browse-live-connection": "{live_browse_connection().unwrap_or_default()}",
            "data-browse-activation-attempts": "{browse_activation_attempts}",
            "data-browse-reply-completions": "{browse_reply_completions}",
            "data-folder-check-completions": "{folder_check_completions}",
            "data-folder-check-pending": "{folder_check_pending}",
            "data-awaiting-woken-connection": "{awaiting_woken_connection}",
            "data-history-activation-attempts": "{history_activation_attempts}",
            "data-remembered-destination-valid": "{remembered_destination_valid}",
            role: "dialog",
            // Browser validity bubbles cannot explain a refusal beside Launch.
            // Submit and the helm keep their ordinary validation authority.
            novalidate: true,
            // Terminal focus, shortcuts and host-result admission inspect this
            // DOM marker independently of inert. A submitted wait owns no modal;
            // hidden quick-launch preparation still does.
            aria_modal: (!submitted_wait()).then_some("true"),
            aria_label: if restart_mode { "Restart a session" } else { "Launch a session" },
            onclick: move |evt: MouseEvent| {
                // A click that leaves the search surface must make its
                // hidden results ineligible before another control handles
                // a following Enter. The search wrapper stops its own
                // events, as does the repository field, so typing and choosing
                // inside either surface keep its result area open.
                composer_search_open.set(false);
                repository_open.set(false);
                evt.stop_propagation();
            },
            oninput: move |_| partial_template_hint.set(false),
            onchange: move |_| partial_template_hint.set(false),
            onkeydown: move |evt: KeyboardEvent| {
                // Native text-field and action-button submission has no choice
                // handler. Keep a held or composing Enter from launching there.
                if evt.key() == Key::Enter && (evt.is_auto_repeating() || evt.is_composing()) {
                    evt.prevent_default();
                }
            },
            on_escape: move |_| {
                if !ops.busy_now() && !template_saving() {
                    if template_panel().is_some() { template_panel.set(None); }
                    else { on_cancel.call(()); }
                }
            },
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                // A launch attempt finishes folder suggestions even when it
                // is refused. The single-column form puts controls below the
                // floating list, so their reachability must not depend on an
                // engine blurring the folder when the request makes it busy.
                folder_open.set(false);
                folder_active.set(None);
                let baseline = automatic_submit_baseline.write().take()
                    .unwrap_or_else(|| launch_activity.snapshot());
                submit_composer.call((submit_consent.0, submit_consent.1, submit_consent.2, baseline, None));
            },
            DialogHeader {
                title: rsx! { if restart_mode { "Restart session" } else if is_replace_with { "Replace session" } else { "New session" } },
                summary: rsx! { if restart_mode { "Resume this session with different settings" } else if is_replace_with { "Start a new session, then delete the source" } else { "Start an agent or a command on any host" } },
                tile: rsx! { crate::icons::ActionIcon { glyph: crate::icons::ActionGlyph::Launch } },
                close: rsx! { button {
                    r#type: "button", class: "dialog-close launch-composer-close",
                    aria_label: "Close session launcher", "data-tooltip": "Close: discard this session setup",
                    disabled: busy || template_saving(),
                    onclick: move |_| {
                        // A queued close cannot unmount the future that claimed
                        // submission before this render disabled the button.
                        if !ops.busy_now() && !template_saving() { on_cancel.call(()); }
                    },
                    crate::icons::ActionIcon { glyph: crate::icons::ActionGlyph::Close }
                } },
            }
            ScrollRegion {
                class: "dialog-body launch-composer-body",
                handle: COMPOSER_DIALOG, selector: ".launch-composer-body", name: "Session setup",
                recovery: RecoveryFocus::new(
                    // A temporarily disabled template name must not let this
                    // body invent a search-focus choice ahead of panel recovery.
                    if template_panel().is_some() { ".save-template-name" }
                    else if attempted_restart { ".create-session-submit" }
                    else if restart_mode { ".launch-composer-cancel" } else { ".launch-composer-search input" },
                    if template_panel().is_some() { None }
                    else if attempted_restart { Some(".launch-composer-cancel") }
                    else { Some(".create-session-submit") },
                    true, RecoveryDelay::OneFrame,
                ),
            if partial_template_hint() && pending_initial_templates.read().is_empty() {
                div { class: "partial-template-banner", role: "alert",
                    onmounted: move |_| {
                        // Count visible time, not catalog-loading time. This
                        // component owns the task, so closing cancels expiry;
                        // the one-shot fence also tolerates a repeated mount event.
                        if !*partial_hint_timer_started.peek() {
                            partial_hint_timer_started.set(true);
                            spawn(async move {
                                crate::reader::sleep_ms(6000).await;
                                partial_template_hint.set(false);
                            });
                        }
                    },
                    strong { "{partial_template_text}" }
                }
            }
            if let Some(notice) = permission_escalation() {
                div {
                    key: "permission-escalation-{notice.generation}",
                    class: "permission-escalation-banner",
                    role: "alert",
                    onanimationend: move |event| {
                        // CSS owns active, unpaused viewing time. Entrance
                        // and descendant animations must not expire recovery.
                        if event.animation_name() == "permission-banner-lifetime"
                            && permission_escalation.peek().as_ref().is_some_and(|current| current.generation == notice.generation)
                        {
                            permission_escalation.set(None);
                        }
                    },
                    crate::icons::ActionIcon { glyph: crate::icons::ActionGlyph::Warning }
                    span { "{notice.message}" }
                    button {
                        r#type: "button", class: "btn btn-neutral permission-escalation-undo",
                        "data-tooltip": "Undo: restore the draft from before this agent switch",
                        disabled: busy,
                        onclick: move |_| {
                            if draft.undo_permission_switch(notice.generation, recovery_targets,
                                permission_escalation, opening_generation, placement_targets) {
                                if let Some((_, captured)) = default_at_switch
                                    .peek()
                                    .as_ref()
                                    .filter(|(captured, _)| *captured == notice.generation)
                                {
                                    restoring_default.set(Some(captured.clone()));
                                }
                                focus_composer_surface();
                            }
                        },
                        "Undo"
                    }
                    button {
                        r#type: "button", class: "btn btn-neutral permission-escalation-dismiss",
                        "data-tooltip": "Dismiss this permission warning",
                        onclick: move |_| {
                            if permission_escalation.peek().as_ref().is_some_and(|current| current.generation == notice.generation) {
                                permission_escalation.set(None);
                                focus_composer_surface();
                            }
                        },
                        "Dismiss"
                    }
                }
            }
            if let Some(message) = preparation_error() {
                p { class: "create-session-error", role: "alert", "{display_peer(&message)}" }
            }
            if loading_setup {
                p { role: "status", "Loading session setup…" }
                button {
                    r#type: "button", class: "btn btn-neutral launch-composer-loading-cancel",
                    "data-tooltip": "Cancel: discard this session setup",
                    onclick: move |_| on_cancel.call(()), "Cancel"
                }
            } else {
            // Why the last template accepted from search was not applied;
            // nothing from it was (SPEC.md: all or nothing).
            if let Some(message) = template_error() {
                p { class: "launch-composer-template-error", role: "status", "{display_peer(&message)}" }
            }
            // Search belongs to the shared shell. A query can choose an
            // agent type, a model, a folder, or a saved setup; it never
            // manufactures a command from text alone, and it never chooses
            // the command tab itself.
            // Destination and its optional name are one draft regardless of
            // which launch controls happen to be active below.
            div {
                div {
                    class: "launch-composer-search",
                    onclick: move |evt| evt.stop_propagation(),
                    input {
                        r#type: "search",
                        role: "combobox",
                        aria_label: "Search folders, harnesses, models, and efforts",
                        aria_expanded: composer_search_open(),
                        aria_controls: "launch-composer-search-results",
                        aria_activedescendant: (composer_search_open() && !search_results_for_keys.is_empty())
                            .then(|| format!("launch-composer-search-option-{}", composer_active_index)),
                        placeholder: "Search names, hosts, folders, harnesses, models…",
                        autocomplete: "off",
                        // Search includes literal host paths and model IDs;
                        // browser text correction would change the query's meaning.
                        autocorrect: "off",
                        autocapitalize: "none",
                        spellcheck: "false",
                        // This node is shared by both modes and mounts once per
                        // dialog. The renderer-level handoff covers engines
                        // where focusing from the parent mount arrives before
                        // the child can accept it, without replaying on later
                        // catalog or history renders.
                        onmounted: move |element| {
                            let input = element.data();
                            if restart_mode { return; }
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
                            choice_targets.promote_environment_history();
                            composer_search.set(evt.value());
                            composer_search_open.set(true);
                            // Preselect the exact-word match for THIS keystroke's
                            // results. Built from the history the promotion above
                            // just installed (read live from both offered snapshots, not
                            // from a render-time clone that predates the promotion),
                            // so the index agrees with the list the next render
                            // draws from the same signal; the clamp at
                            // `composer_active_index` covers whatever still moves
                            // between this keystroke and a later render.
                            let promoted_history = offered_recent_history(
                                offered_history.peek().as_ref(),
                                create_target().as_ref(),
                                live_destination.peek().as_ref(),
                                offered_environment_history.peek().as_deref().unwrap_or_default(),
                            );
                            let active_harness = (draft.snapshot().tab()
                                == LaunchTab::Agent)
                                .then(&*structured_harness)
                                .flatten();
                            let active_model = (draft.snapshot().tab()
                                == LaunchTab::Agent)
                                .then(&*structured_model)
                                .flatten();
                            let mut rows = crate::launch_composer::search_results(
                                draft.snapshot().tab() == LaunchTab::Agent && active_harness.is_none(),
                                &promoted_history,
                                &catalog_models_for_search_input,
                                &evt.value(),
                                active_harness,
                                active_model.as_deref(),
                            );
                            rows.extend(crate::launch_composer::name_host_search_results(
                                &evt.value(),
                                &composer_hosts_for_search_input,
                            ));
                            rows.extend(crate::launch_composer::template_search_results(
                                &evt.value(),
                                &template_names_for_search_input,
                            ));
                            let query = evt.value();
                            let (scope, value) = crate::launch_composer::scoped_query(&query);
                            if scope == crate::launch_composer::SearchScope::Github {
                                // A discovery answer belongs to its exact query and
                                // authority. A new owner/repo still offers its manual
                                // row immediately, without borrowing old discoveries.
                                let discovered = repositories_for_search_input.as_ref()
                                    .filter(|(authority, _)| authority.query == value
                                        && Some(authority) == live_repository_authority.peek().as_ref())
                                    .map_or(&[][..], |(_, repos)| repos.as_slice());
                                rows.extend(repository_choices(value, discovered).into_iter()
                                    .map(crate::launch_composer::ComposerSearchResult::Github));
                            }
                            let rows = crate::launch_composer::finish_search_results(
                                &query, rows, draft.snapshot().tab() == LaunchTab::Agent && active_harness.is_none(),
                            );
                            let groups = crate::launch_composer::grouped_search_results(rows);
                            composer_search_index.set(
                                crate::launch_composer::default_search_index(&groups, &evt.value()),
                            );
                        },
                        onkeydown: {
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
                                    // Step from the CLAMPED index (the row the DOM is
                                    // highlighting), not the raw signal: after the list
                                    // shrank, the raw value can exceed the length and the
                                    // modulo would land on an arbitrary row instead of
                                    // wrapping from the last row to the first.
                                    let next = (composer_active_index + 1) % search_results_for_keys.len();
                                    composer_search_index.set(next);
                                    scroll_composer_search_result(next);
                                }
                                Key::ArrowUp if composer_search_open() && !search_results_for_keys.is_empty() => {
                                    evt.prevent_default();
                                    let previous = (composer_active_index + search_results_for_keys.len() - 1)
                                        % search_results_for_keys.len();
                                    composer_search_index.set(previous);
                                    scroll_composer_search_result(previous);
                                }
                                // While the combobox holds a query, Enter belongs to search
                                // even when the query has no matches. Otherwise a no-result
                                // query would bubble to the form and launch whatever stale
                                // selection the composer happened to hold.
                                //
                                // An empty query submits normally. Launch uses
                                // aria-disabled so an incomplete setup reaches
                                // submit and produces words beside that button.
                                // Literal emptiness matters: spaces still form
                                // a query and must never launch stale choices.
                                Key::Enter if !evt.is_composing() => {
                                    // Step aside entirely, default included, while a sequence waits
                                    // for this Enter; accepting a search row here would also rename
                                    // the session from the box's text and clear it.
                                    if launch_sequence.is_some_and(|sequence| sequence.read().is_some()) {
                                        return;
                                    }
                                    // A HELD Enter must not launch. Accepting a result
                                    // empties the box synchronously, so the key's OS
                                    // auto-repeat (tens of milliseconds later) would
                                    // otherwise arrive at an empty box and fall through
                                    // to implicit submission — turning "type the last
                                    // word, press Enter a beat too long" into a launch
                                    // nobody asked for. Inside the arm, not as a match
                                    // guard: a failed guard would fall to `_ => {}` with
                                    // the default un-prevented, which is the submit.
                                    if evt.is_auto_repeating() {
                                        evt.prevent_default();
                                        return;
                                    }
                                    if composer_search().is_empty() {
                                        return;
                                    }
                                    evt.prevent_default();
                                    if !draft_transition_allowed(ops) {
                                        template_error.set(Some("Wait for the current operation to finish before choosing a result".into()));
                                        return;
                                    }
                                    // A queued key can observe a newer input signal
                                    // before its rows render. Reveal that query first;
                                    // never accept a stale row under different text.
                                    if composer_search() != search_query_for_keys {
                                        composer_search_open.set(true);
                                        return;
                                    }
                                    let active = search_results_for_keys.get(composer_active_index)
                                        .map(|_| composer_active_index);
                                    match crate::launch_composer::search_enter_target(composer_search_open(), active) {
                                        crate::launch_composer::SearchEnterTarget::Pick(index) => {
                                            accept_search.call((search_results_for_keys[index].clone(), None, false));
                                        }
                                        crate::launch_composer::SearchEnterTarget::Reopen => composer_search_open.set(true),
                                        crate::launch_composer::SearchEnterTarget::Nothing => {},
                                    }
                                }
                                _ => {}
                            }
                        }
                        },
                    }
                    if composer_search_open() && (!search_result_groups.is_empty() || search_message.is_some()) {
                        div {
                            id: "launch-composer-search-results",
                            role: if search_result_groups.is_empty() { "status" } else { "listbox" },
                            class: "launch-composer-search-results",
                            if let Some(message) = search_message {
                                div { class: message.class(), "{message.text}" }
                            }
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
                                                if matches!(result, crate::launch_composer::ComposerSearchResult::Permissions(crate::launch_composer::ComposerPermission::Guarded)) {
                                                    span { class: "visually-hidden", id: "launch-composer-search-option-{index}-help",
                                                        {search_result_tooltip(&result, &match &result {crate::launch_composer::ComposerSearchResult::Recent(entry)=>crate::launch_composer::recent_placement_label(entry,&selected_host_label,&environment_places),_=>selected_host_label.clone()}, *structured_harness.read())}
                                                    }
                                                }
                                                button {
                                                    id: "launch-composer-search-option-{index}",
                                                    r#type: "button",
                                                    role: "option",
                                                    dir: "ltr",
                                                    aria_selected: composer_active_index == index,
                                                    class: if matches!(result, crate::launch_composer::ComposerSearchResult::Recent(_)) {
                                                        if composer_active_index == index { "launch-composer-search-recent selected" } else { "launch-composer-search-recent" }
                                                    } else if composer_active_index == index { "selected" } else { "" },
                                                    "data-tooltip": search_result_tooltip(&result, &match &result {crate::launch_composer::ComposerSearchResult::Recent(entry)=>crate::launch_composer::recent_placement_label(entry,&selected_host_label,&environment_places),_=>selected_host_label.clone()}, *structured_harness.read()),
                                                    "data-tooltip-paragraphs": if matches!(result, crate::launch_composer::ComposerSearchResult::Permissions(crate::launch_composer::ComposerPermission::Guarded)) { "true" } else { "false" },
                                                    aria_describedby: if matches!(result, crate::launch_composer::ComposerSearchResult::Permissions(crate::launch_composer::ComposerPermission::Guarded)) { format!("launch-composer-search-option-{index}-help") } else { String::new() },
                                                    // Search recents use two visual spans as ordinary
                                                    // recents do. Their accessible label repeats the
                                                    // complete title with the result kind, instead of
                                                    // losing the separator where those spans meet.
                                                    aria_label: match &result {
                                                        crate::launch_composer::ComposerSearchResult::Recent(entry) => format!(
                                                            "Recent setup: {} · {} · {}",
                                                                display_peer(&crate::launch_composer::recent_destination_label(entry)), crate::launch_composer::recent_placement_label(entry,&selected_host_label,&environment_places),
                                                            display_peer(&crate::launch_composer::selection_summary(&entry.selection)),
                                                        ),
                                                        _ => String::new(),
                                                    },
                                                    onclick: {
                                                        let result = result.clone();
                                                        move |_| {
                                                            accept_search.call((result.clone(), None, false));
                                                        }
                                                    },
                                                    match &result {
                                                        crate::launch_composer::ComposerSearchResult::Name(name) => rsx! { "Set session name: {display_peer(name)}" },
                                                        crate::launch_composer::ComposerSearchResult::Host(host) => rsx! { "Host: {host.label}" },
                                                        // Folder actions alter only cwd; a recent
                                                        // setup visibly names every choice it owns.
                                                        // Keep paths isolated and mono without giving
                                                        // the interface's action label that font role.
                                                        crate::launch_composer::ComposerSearchResult::UsePath(folder)
                                                        | crate::launch_composer::ComposerSearchResult::Folder(folder) => rsx! {
                                                            "Use this path: " span { class: "peer-value font-mono", dir: "ltr", "{display_peer(folder)}" }
                                                        },
                                                        crate::launch_composer::ComposerSearchResult::BrowsePath(folder) => rsx! {
                                                            "Browse this path: " span { class: "peer-value font-mono", dir: "ltr", "{display_peer(folder)}" }
                                                        },
                                                        crate::launch_composer::ComposerSearchResult::Harness(harness) => rsx! { "Harness: {crate::launch_composer::harness_label(*harness)}" },
                                                        crate::launch_composer::ComposerSearchResult::Template(name) => rsx! { "Template: {display_peer(name)}" },
                                                        crate::launch_composer::ComposerSearchResult::Github(repo) => rsx! { "Managed checkout: {display_peer(&repo.identifier())}" },
                                                        crate::launch_composer::ComposerSearchResult::Model { id, harness } => rsx! { "Model: {display_peer(id)} ({crate::launch_composer::harness_label(*harness)})" },
                                                        crate::launch_composer::ComposerSearchResult::Effort(effort) => rsx! {
                                                            "Effort: {crate::launch_composer::effort_label(*effort)}"
                                                        },
                                                        crate::launch_composer::ComposerSearchResult::Permissions(permission) => rsx! {
                                                            "Permissions: "
                                                            {match permission {
                                                                crate::launch_composer::ComposerPermission::Guarded => "Guarded",
                                                                crate::launch_composer::ComposerPermission::OwnSetting => "Agent's own setting",
                                                                crate::launch_composer::ComposerPermission::Yolo => "YOLO",
                                                                crate::launch_composer::ComposerPermission::Custom => "Custom",
                                                                crate::launch_composer::ComposerPermission::Approve => crate::launch_composer::permission_display_value(LaunchPermission::Approve),
                                                                crate::launch_composer::ComposerPermission::SmartApprove => crate::launch_composer::permission_display_value(LaunchPermission::SmartApprove),
                                                                crate::launch_composer::ComposerPermission::Chat => crate::launch_composer::permission_display_value(LaunchPermission::Chat),
                                                            }}
                                                        },
                                                        crate::launch_composer::ComposerSearchResult::Recent(entry) => rsx! {
                                                            span { class: "launch-composer-search-recent-destination",
                                                                "Recent setup: "
                                                                span { class: if entry.github_repo.is_none() { "font-mono" } else { "" },
                                                                    "{display_peer(&crate::launch_composer::recent_destination_label(&entry))}"
                                                                }
                                                                " · {crate::launch_composer::recent_placement_label(entry,&selected_host_label,&environment_places)}"
                                                            }
                                                            // Keep the dangerous permission as a semantic span
                                                            // instead of flattening it into the shared summary
                                                            // string, so search recents carry the same warning as
                                                            // the visible recent-setup rows.
                                                            span { class: "launch-composer-search-recent-selection",
                                                                "{display_peer(&crate::launch_composer::selection_summary_before_permissions(&entry.selection))} · Permissions: "
                                                                if crate::launch_composer::normalized_permissions(entry.selection.harness, entry.selection.permissions) == Some(LaunchPermission::Yolo) {
                                                                    span { class: "launch-composer-danger", "YOLO" }
                                                                } else {
                                                                    "{crate::launch_composer::selection_permission_value(&entry.selection)}"
                                                                }
                                                            }
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
                // A direct recent owns destination and agent fields. Restart
                // holds those controls; search still explains a rejected recent.
                if !restart_mode && !recent_launches.is_empty() {
                    div { class: "launch-composer-recents",
                            span { class: "launch-composer-section-label", "Recent setups" }
                            div { class: "launch-composer-recent-slots",
                            for (entry, summary, explicit, permission, uses_guarded_default) in recent_launches.iter().take(3).map(|entry| (
                                entry,
                                crate::launch_composer::selection_summary(&entry.selection),
                                crate::launch_composer::selection_explicit_before_permissions(&entry.selection).join(" · "),
                                crate::launch_composer::selection_permission_value(&entry.selection),
                                entry.selection.harness.omitted_permission() == Some(LaunchPermission::Guarded)
                                    && entry.selection.harness.request_permission(entry.selection.permissions) == Some(LaunchPermission::Guarded),
                            )) {
                                button {
                                    r#type: "button",
                                    dir: "ltr",
                                    disabled: busy,
                                    "data-tooltip": "{display_peer(&crate::launch_composer::recent_destination_label(&entry))} · {crate::launch_composer::recent_placement_label(entry,&selected_host_label,&environment_places)} · {display_peer(&summary)}",
                                    // The visible row makes scanning cheaper, but
                                    // its tooltip and accessible name retain every
                                    // saved value that the one-line layout may
                                    // truncate. The punctuated string also keeps
                                    // adjacent spans from running together for
                                    // assistive technology.
                                    // In Replace with, Enter here submits the
                                    // replace, and this name stands in for the
                                    // row's visible "⏎ replace" hint, so it
                                    // leads with the verb as the button's own
                                    // name does; the row also points at the
                                    // same warning the replace button does.
                                    aria_label: "{recent_name_prefix}{display_peer(&crate::launch_composer::recent_destination_label(&entry))} · {crate::launch_composer::recent_placement_label(entry,&selected_host_label,&environment_places)} · {display_peer(&summary)}",
                                    aria_describedby: replace_warning.as_ref().map(|_| REPLACE_WARNING_ID),
                                    onclick: {
                                        let entry = entry.clone();
                                        move |_| {
                                            apply_recent.call(entry.clone());
                                        }
                                    },
                                    onkeydown: {
                                        let entry = entry.clone();
                                        move |evt| {
                                            if evt.key() != Key::Enter {
                                                return;
                                            }
                                            // Native button activation would emit a
                                            // synthetic click after Enter. Suppress it
                                            // so this row fills and submits once.
                                            evt.prevent_default();
                                            // This row submits explicitly, before a
                                            // bubbled event reaches the form guard.
                                            // Reject a held key here even after a
                                            // fast refusal released the busy lock.
                                            if evt.is_auto_repeating() || evt.is_composing() {
                                                return;
                                            }
                                            if !apply_recent.call(entry.clone()) {
                                                return;
                                            }
                                            // A launch, so in Replace with it
                                            // carries this render's precondition;
                                            // see `launch_from_choice` for why it
                                            // submits directly.
                                            submit_answer.call(submit_consent);
                                        }
                                    },
                                    // The row is a grid so that harness, folder,
                                    // host, and choices line up as columns down
                                    // the list (see `.launch-composer-recent-slots
                                    // > button` in app.css). The folder and host
                                    // are separate cells, but they stay children
                                    // of ONE destination span, separator included,
                                    // so that span's text is still "folder · host":
                                    // the browser suite reads it, and the span
                                    // turns into its children for layout
                                    // (`display: contents`). The separators are
                                    // real text for that reason and hidden by the
                                    // stylesheet; the columns are what separate
                                    // the cells on screen.
                                    span { class: "launch-composer-recent-harness", "{crate::launch_composer::harness_label(entry.selection.harness)}" }
                                    span { class: "launch-composer-recent-destination", dir: "ltr",
                                        span { class: "launch-composer-recent-folder", "{display_peer(&crate::launch_composer::recent_destination_label(&entry))}" }
                                        span { class: "launch-composer-recent-separator", " · " }
                                        span { class: "launch-composer-recent-host", "{crate::launch_composer::recent_placement_label(entry,&selected_host_label,&environment_places)}" }
                                    }
                                    // Only what this setup sets explicitly, so
                                    // the row that differs stands out; the title
                                    // and accessible name above still name every
                                    // default (`selection_explicit_before_permissions`
                                    // has the reasoning). The permission is
                                    // spelled here, not by a summary helper, so
                                    // the one danger-colored word cannot drift
                                    // from the text around it the way string
                                    // surgery on a Debug spelling would.
                                    span { class: "launch-composer-recent-selection",
                                        // Only a harness's guarded request default can be
                                        // compacted into "defaults". OMP's guarded write
                                        // choice is explicit because its default is YOLO;
                                        // own-setting must also retain its uncertainty.
                                        if explicit.is_empty() && uses_guarded_default {
                                            "{crate::launch_composer::RECENT_ALL_DEFAULTS}"
                                        }
                                        // Guarded, not interpolated bare:
                                        // `display_peer` renders an empty
                                        // string as the word "(empty)", which
                                        // is right for a peer-authored field
                                        // and wrong for "nothing to list".
                                        if !explicit.is_empty() {
                                            "{display_peer(&explicit)}"
                                        }
                                        if !uses_guarded_default {
                                            if !explicit.is_empty() { " · " }
                                            "Permissions: "
                                            if crate::launch_composer::normalized_permissions(entry.selection.harness, entry.selection.permissions) == Some(LaunchPermission::Yolo) {
                                                span { class: "launch-composer-danger", "YOLO" }
                                            } else {
                                                "{permission}"
                                            }
                                        }
                                    }
                                    span { class: "launch-composer-recent-hint", "⏎ {submit_verb}" }
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
            // Destination rows precede the launch-kind fields in both visual
            // and native Tab order. Switching tabs only swaps the latter.
            if search_owns_repositories && !show_checkout_root_setup {
                if let Some((note, warning)) = &repository_note {
                    div { class: if *warning { "launch-composer-repository-note launch-composer-search-warning" } else { "launch-composer-repository-note" }, role: "status", "{display_peer(note)}" }
                }
            }
            if search_scope == crate::launch_composer::SearchScope::Template {
                if let Some(message) = &templates_read_error {
                    p { class: "create-catalog-error launch-composer-templates-error",
                        span { role: "status", "Couldn't load templates: {display_peer(message)} " }
                        button {
                            r#type: "button",
                            class: "btn btn-neutral",
                            onclick: move |_| {
                                launch_templates.clear();
                                launch_templates.restart();
                            },
                            "Retry"
                        }
                    }
                }
            }
            div { class: "launch-composer-columns",
                    if restart_mode {
                        div { class: "launch-composer-held-context",
                            p { "Agent type (held): " span { class: "peer-value", dir: "ltr", "{held_agent_label}" } }
                            p { "Host (held): " span { class: "peer-value", dir: "ltr", "{selected_host_label}" } }
                            p { "Folder (held): " span { class: "peer-value font-mono", dir: "ltr", "{summary_folder}" } }
                            if let Some(notice) = folder_notice() { PeerLine { class: "launch-composer-folder-warning".to_string(), parts: notice } }
                        }
                    } else {
                    div { class: "launch-composer-column-destination",
                        // A destination is a host and its folder or managed checkout.
                        // Keep that type choice, host reconciliation and the active
                        // type's editor together. Folder recents belong only to the
                        // folder editor; a repository owns its verified preview and
                        // explanation instead. The optional name follows both. This makes a
                        // launch's location reviewable without restoring a second
                        // summary of its choices.
                        div { class: "launch-composer-destination",
                            span { class: "launch-composer-section-label", "Destination" }
                            div { class: "launch-composer-row",
                                span { class: "launch-composer-row-label", "Where" }
                            DestinationTypeControl { managed: checkout_mode, disabled: busy,
                                on_change: move |managed| {
                                    draft.choose_destination_type(managed, placement_targets, repository_open);
                                },
                            }
                            }
                            div { class: "launch-composer-placement-row",
                            div { class: "launch-composer-destination-row",
                                {host_select.clone()}
                                if !checkout_mode && recipe().is_none() {
                                    button {
                                        r#type: "button",
                                        disabled: busy || selected.is_none(),
                                        aria_label: "Browse this path",
                                        class: "btn btn-neutral",
                                        "data-tooltip": "Browse: list the folders under this path on the host",
                                        onclick: move |_| {
                                            if !draft_transition_allowed(ops) { return; }
                                            request_directory_browse(
                                                browse_base_for_folder.clone(), selected, &browse_hosts_for_folder, browse_target,
                                                draft.snapshot().submitted_cwd(),
                                                browse_generation, browse_request, browse_result, browse_error, browse_reply_completions,
                                                live_browse_connection, draft,
                                            );
                                        },
                                        "Browse"
                                    }
                                }
                            }
                            }
                            {host_notes.clone()}
                            if recipe().is_some() {
                                p { class: "launch-composer-checkout-explanation",
                                    {environment_target().map(|target| crate::environment_ui::ProviderUi(target.provider()).launch_help()).unwrap_or("")}
                                }
                                if environment_target().is_some_and(|target|target.provider().asks_before_creating())
                                    && let Some(Ok((_, settings))) = &sandbox_answer
                                    && let farhelm_proto::tensorlake::SshKeyStatus::Ready { register_command, .. } = &settings.ssh_key {
                                    p { "Register Farhelm's SSH key with Tensorlake before launching. Farhelm cannot check account registration." }
                                    code { "{display_peer(register_command)}" }
                                    button { r#type: "button", class: "btn btn-neutral",
                                        "data-tooltip": "Copy the command that registers Farhelm's SSH key with your Tensorlake account",
                                        onclick: { let command = register_command.clone(); move |_| crate::session_view::copy_to_clipboard(&command) },
                                        "Copy registration command" }
                                }
                            }
                            if show_checkout_root_setup {
                                crate::checkout_root::CheckoutRootField {
                                    note: "Sets the default checkout folder for every host without its own. ~ means each host's home folder. Change the default later in Settings → Checkouts, or set a host's own folder in its settings.",
                                    on_saved: move |_| {
                                        // Refresh both immediately after explicit setup,
                                        // without waiting for history's next configuration
                                        // notification to make this launcher usable.
                                        repository_response.restart();
                                        preview_revision.with_mut(|revision| *revision = revision.wrapping_add(1));
                                    },
                                }
                            }
                            if checkout_mode {
                                label { class: "launch-composer-repository-field",
                                    onclick: move |evt| {
                                        composer_search_open.set(false);
                                        evt.stop_propagation();
                                    },
                                    span { class: "launch-composer-row-label", "Repository" }
                                    input { r#type: "text", role: "combobox", aria_label: "Repository", aria_autocomplete: "list",
                                        aria_expanded: !repository_read_only && repository_open(),
                                        aria_controls: (!repository_read_only && repository_open()).then_some("checkout-repository-options"),
                                        aria_activedescendant: (!repository_read_only && repository_open() && !repository_suggestions.is_empty()).then(|| format!("checkout-repository-{repository_active}")),
                                        value: "{repository_field_value}", readonly: repository_read_only,
                                        placeholder: "owner/repo", disabled: busy,
                                        autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false", dir: "ltr",
                                        onfocus: move |_| { if !repository_read_only { repository_open.set(true); } },
                                        onblur: move |_| repository_open.set(false),
                                        oninput: move |evt| {
                                            draft.edit_repository_query(evt.value(), repository_read_only, placement_targets, repository_targets);
                                        },
                                        onkeydown: move |evt: KeyboardEvent| {
                                            match evt.key() {
                                                Key::Escape if repository_open() => {
                                                    evt.prevent_default(); evt.stop_propagation(); repository_open.set(false);
                                                }
                                                Key::ArrowDown | Key::ArrowUp if repository_open() && !repository_suggestions_for_keys.is_empty() => {
                                                    evt.prevent_default();
                                                    let count = repository_suggestions_for_keys.len();
                                                    repository_index.set(if evt.key() == Key::ArrowDown { (repository_active + 1) % count } else { (repository_active + count - 1) % count });
                                                }
                                                Key::Enter => {
                                                    evt.prevent_default(); evt.stop_propagation();
                                                    if evt.is_auto_repeating() || evt.is_composing() || repository_read_only { return; }
                                                    if recipe().is_some() {
                                                        match GithubRepo::parse(&draft.snapshot().destination().repository_text()) {
                                                            Ok(repo) => { select_repository.call(repo); repository_open.set(false); }
                                                            Err(reason) => error.set(Some(reason)),
                                                        }
                                                        return;
                                                    }
                                                    if !draft_transition_allowed(ops) {
                                                        template_error.set(Some("Wait for the current operation to finish before choosing a repository".into()));
                                                        return;
                                                    }
                                                    // A queued key must not select a row from
                                                    // the previous query or host installation.
                                                    if draft.snapshot().destination().repository_text() != repository_query_for_keys
                                                        || *live_repository_authority.peek() != repository_authority_for_keys
                                                    {
                                                        repository_open.set(true);
                                                        return;
                                                    }
                                                    match crate::launch_composer::search_enter_target(
                                                        repository_open(),
                                                        (!repository_suggestions_for_keys.is_empty()).then_some(repository_active),
                                                    ) {
                                                        crate::launch_composer::SearchEnterTarget::Pick(index) => {
                                                            if let Some(repo) = repository_suggestions_for_keys.get(index) {
                                                                select_repository.call(repo.clone());
                                                            }
                                                        }
                                                        crate::launch_composer::SearchEnterTarget::Reopen => repository_open.set(true),
                                                        crate::launch_composer::SearchEnterTarget::Nothing => {}
                                                    }
                                                }
                                                _ => {}
                                            }
                                        },
                                    }
                                }
                                if repository_read_only {
                                    p { "Invisible formatting is shown escaped. Clear the repository to replace it." }
                                    // Drop selection and preview together, as an ordinary
                                    // repository edit does, without copying display markers.
                                    button { r#type: "button", class: "btn btn-neutral launch-composer-repository-clear", disabled: busy,
                                        "data-tooltip": "Clear repository: remove this selection before editing",
                                        onclick: move |_| {
                                            draft.clear_repository(placement_targets, repository_targets);
                                        },
                                        "Clear repository"
                                    }
                                }
                                if !repository_read_only && repository_open() {
                                    div { class: "launch-composer-repository-options", id: "checkout-repository-options",
                        role: if repository_suggestions.is_empty() { "status" } else { "listbox" }, aria_label: "Repositories",
                                        onclick: move |evt| {
                                            composer_search_open.set(false);
                                            evt.stop_propagation();
                                        },
                                        if repository_suggestions.is_empty() && let Some(message) = repository_message {
                                            div { class: message.class(), "{message.text}" }
                                        }
                                        for (index, repo) in repository_suggestions.iter().enumerate() {
                                            button { r#type: "button", role: "option", id: "checkout-repository-{index}",
                                                aria_selected: index == repository_active, disabled: busy, tabindex: "-1",
                                                "data-tooltip": "Managed checkout of {display_peer(&repo.identifier())}",
                                                onmousedown: move |evt| evt.prevent_default(),
                                                onclick: { let repo = repo.clone(); move |_| {
                                                    select_repository.call(repo.clone());
                                                } },
                                                crate::icons::BranchIcon {}
                                                "{display_peer(&repo.identifier())}"
                                            }
                                        }
                                    }
                                }
                                if choosing_repository && !search_owns_repositories && !show_checkout_root_setup && let Some((note, warning)) = &repository_note {
                                    div { class: if *warning { "launch-composer-repository-note launch-composer-search-warning" } else { "launch-composer-repository-note" }, role: "status", "{display_peer(note)}" }
                                }
                                if let DestinationDraft::Github { repo } = destination_draft() {
                                    div { class: "launch-composer-checkout-preview", aria_live: "polite",
                                        // Completion must be observable even while reconciliation
                                        // hides a preview error behind the original accepted path.
                                        "data-preview-state": match preview_state() {
                                            PreviewState::Pending => "pending",
                                            PreviewState::Ready { .. } => "ready",
                                            PreviewState::Failed { .. } => "failed",
                                        },
                                        "Managed checkout of {display_peer(&repo.identifier())} on {selected_host_label}"
                                        if allocate_after_resume {
                                            div { "{allocation_notice}" }
                                        } else if let Some(preview) = &displayed_preview {
                                            div { class: "font-mono", dir: "ltr", "{preview.cwd.escaped()}" }
                                        }
                                        if allocate_after_resume {
                                            // The old preview's pending/failure state says
                                            // nothing about allocation after the host wakes.
                                        } else if retry_binding.is_some() {
                                            div { "Retry reconciles the original request at this path" }
                                        } else {
                                            match preview_state() {
                                                PreviewState::Pending => rsx! { div { "Waiting for a current checkout preview" } },
                                                PreviewState::Failed { message, .. } => rsx! { div { class: "create-session-error", "{display_peer(&message)}" } },
                                                PreviewState::Ready { .. } => rsx! {},
                                            }
                                        }
                                    }
                                }
                                p { class: "launch-composer-checkout-explanation",
                                    if recipe().is_some() { "Farhelm clones the repository in the new environment. Delete permanently removes it, including the checkout and unpushed changes." }
                                    else { "Farhelm clones the repository into a new folder named after the session, under this host's checkout folder. When the last session using it is deleted, the folder moves to the trash." }
                                }
                            }
                            div { class: "launch-composer-row",
                            span { class: "launch-composer-row-label", "Folder" }
                            div { class: "launch-composer-folder-picker",
                            input {
                                class: "font-mono",
                                r#type: "text",
                                required: !checkout_mode,
                                readonly: checkout_mode || recipe().is_some(),
                                role: (!checkout_mode).then_some("combobox"),
                                aria_autocomplete: (!checkout_mode).then_some("list"),
                                aria_expanded: (!checkout_mode).then_some(folder_open()),
                                aria_controls: (!checkout_mode && folder_open()).then_some("launcher-folder-options"),
                                aria_activedescendant: if !checkout_mode && folder_open() { folder_highlight.map(|index| format!("launcher-folder-{index}")) } else { None },
                                autocomplete: "off",
                                autocorrect: "off",
                                autocapitalize: "none",
                                spellcheck: "false",
                                dir: "ltr",
                                value: "{folder_field_value}",
                                placeholder: "{folder_placeholder}",
                                disabled: busy,
                                aria_label: "Folder",
                                aria_invalid: folder_notice().is_some().then_some("true"),
                                aria_describedby: folder_notice().is_some().then_some("launcher-folder-problem"),
                                onfocus: move |_| {
                                    if recipe().is_none() && !draft.snapshot().destination().is_managed() {
                                        folder_active.set(None);
                                        folder_open.set(true);
                                    }
                                },
                                onblur: move |_| {
                                    folder_open.set(false);
                                    folder_active.set(None);
                                },
                                onkeydown: move |evt: KeyboardEvent| {
                                    if recipe().is_some() || draft.snapshot().destination().is_managed() || evt.is_composing() { return; }
                                    match evt.key() {
                                        Key::Escape if folder_open() => {
                                            evt.prevent_default(); evt.stop_propagation();
                                            folder_open.set(false); folder_active.set(None);
                                        }
                                        Key::Tab => { folder_open.set(false); folder_active.set(None); }
                                        Key::ArrowDown | Key::ArrowUp => {
                                            evt.prevent_default(); evt.stop_propagation();
                                            if !draft_transition_allowed(ops) { return; }
                                            let count = folder_choices_for_keys.len();
                                            let current = if folder_open() { folder_highlight } else { None };
                                            let next = match (current, evt.key()) {
                                                (Some(index), Key::ArrowDown) => (index + 1) % count,
                                                (Some(index), _) => (index + count - 1) % count,
                                                (None, Key::ArrowDown) => 0,
                                                (None, _) => count - 1,
                                            };
                                            folder_active.set(Some(next));
                                            folder_open.set(true);
                                        }
                                        Key::Enter if folder_open() && folder_highlight.is_some() => {
                                            evt.prevent_default(); evt.stop_propagation();
                                            if evt.is_auto_repeating() { return; }
                                            if let Some(choice) = folder_highlight.and_then(|index| folder_choices_for_keys.get(index)) {
                                                choose_folder.call(choice.clone());
                                            }
                                        }
                                        _ => {}
                                    }
                                },
                                oninput: move |evt| {
                                    draft.edit_folder(evt.value(), placement_targets, folder_targets);
                                },
                            }
                            // Float like model/search suggestions. In-flow history
                            // would resize and recenter this dialog when pointer blur
                            // closes it, moving the pressed action before mouseup.
                            if recipe().is_none() && !checkout_mode && folder_open() {
                                div { class: "launch-composer-folder-options", id: "launcher-folder-options", role: "listbox", aria_label: "Folders",
                                    // A press anywhere in the list (a heading, the empty
                                    // row, a gap, its scrollbar) must not take focus
                                    // from the field, whose blur would close the list.
                                    onmousedown: move |evt| evt.prevent_default(),
                                    div { role: "group", aria_labelledby: "launcher-folder-recent",
                                        div { class: "launch-composer-folder-heading", id: "launcher-folder-recent", "Recent" }
                                        if folder_recents.is_empty() {
                                            div { class: "launch-composer-folder-empty", "No recent folders" }
                                        }
                                        for (index, folder) in folder_recents.iter().enumerate() {
                                            button { r#type: "button", role: "option", id: "launcher-folder-{index}", tabindex: "-1",
                                                aria_selected: folder_highlight == Some(index), disabled: busy,
                                                aria_label: "{folder.display_cwd.escaped()}",
                                                "data-tooltip": "{folder.display_cwd.escaped()}",
                                                onmousedown: move |evt| evt.prevent_default(),
                                                onclick: { let choice = folder_choices[index].clone(); move |_| choose_folder.call(choice.clone()) },
                                                span { class: "launch-composer-folder-path", dir: "ltr", span { "{folder_tail_label(&folder.display_cwd)}" } }
                                            }
                                        }
                                    }
                                    div { role: "group", aria_labelledby: "launcher-folder-reset",
                                        div { class: "launch-composer-folder-heading", id: "launcher-folder-reset", "Reset" }
                                        button { r#type: "button", role: "option", id: "launcher-folder-{folder_recents.len()}", tabindex: "-1",
                                            aria_selected: folder_highlight == Some(folder_recents.len()), disabled: busy,
                                            aria_label: "Reset folder to home", "data-tooltip": "Home: start in your home folder on this host",
                                            onmousedown: move |evt| evt.prevent_default(), onclick: move |_| choose_folder.call(FolderChoice::Home),
                                            span { "Home" } span { class: "launch-composer-folder-meaning", "~ on this host" }
                                        }
                                        button { r#type: "button", role: "option", id: "launcher-folder-{folder_recents.len() + 1}", tabindex: "-1",
                                            aria_selected: folder_highlight == Some(folder_recents.len() + 1), disabled: busy,
                                            aria_label: "Reset destination to local home", "data-tooltip": "Local home: start in your home folder on this machine",
                                            onmousedown: move |evt| evt.prevent_default(), onclick: move |_| choose_folder.call(FolderChoice::LocalHome),
                                            span { "Local home" } span { class: "launch-composer-folder-meaning", "~ on this machine" }
                                        }
                                    }
                                }
                            }
                            }
                            }
                            if let Some(notice) = folder_notice() {
                                div { id: "launcher-folder-problem",
                                    PeerLine { class: "launch-composer-folder-problem".to_string(), parts: notice }
                                }
                            }
                            // The optional name belongs to the shared destination
                            // block. Keeping one mounted input gives assistive
                            // technology one destination and preserves strict label
                            // lookup.
                            label { class: "launch-composer-name",
                                // Keep the optional-name label associated with its
                                // single input while the field joins the row grid.
                                span { class: "launch-composer-name-caption",
                                    span { class: "launch-composer-row-label", "Name (optional)" }
                                    if template_base.is_some() {
                                        span { class: "launch-composer-name-policy", "data-tooltip": "This template supplies a base name; Farhelm adds a number when its managed-checkout folder is already taken", "From template · numbered if taken" }
                                    }
                                }
                                input {
                                    r#type: "text",
                                    autocomplete: "off",
                                    autocorrect: "off",
                                    autocapitalize: "none",
                                    spellcheck: "false",
                                    // A clone can seed this from a peer-supplied
                                    // title, using the same escaped-display/raw-seed
                                    // model as the folder field above. A fresh
                                    // checkout can hide that seed; see
                                    // `title_field_value`.
                                    dir: "ltr",
                                    aria_label: "Name (optional)",
                                    value: "{title_field_value}",
                                    placeholder: "{title_placeholder}",
                                    disabled: busy,
                                    oninput: move |evt| {
                                        // Ownership still changes on an equal edit; only
                                        // a different submitted name retires its retry.
                                        draft.edit_title(evt.value());
                                    },
                                }
                                if let Some((base, true)) = &template_base {
                                    span { class: "launch-composer-name-taken", "data-tooltip": "The template's base checkout name is already taken; this launch uses the first available numbered name", "(" bdi { dir: "ltr", "{display_peer(base)}" } " is taken)" }
                                }
                            }
                        }
                    }
                    }
                    div { class: "launch-composer-column-choices",
                        // One tab per launch kind. Switching only changes
                        // which draft is shown and submitted; each tab keeps
                        // its own (see `LaunchTab`).
                        // The tab pattern's keyboard half: one tab stop (the
                        // selected tab), and the arrow, Home and End keys
                        // select and focus the other tab, as screen readers
                        // announcing "tab, 1 of 2" lead people to expect.
                        if !restart_mode {
                        div { class: "launch-kind-tabs", role: "tablist", "aria-label": "Launch kind",
                            for (tab, label) in [(LaunchTab::Agent, "Agent"), (LaunchTab::Command, "Command")] {
                                button {
                                    r#type: "button",
                                    role: "tab",
                                    aria_label: if tab == LaunchTab::Agent { "Agent" } else { "Command" },
                                    class: if launch_tab() == tab { "launch-kind-tab selected" } else { "launch-kind-tab" },
                                    "data-tooltip": if tab == LaunchTab::Agent { "Agent: start a supported agent with its model, effort and permissions" } else { "Command: run a command line you type, as written" },
                                    "aria-selected": "{launch_tab() == tab}",
                                    tabindex: if launch_tab() == tab { "0" } else { "-1" },
                                    disabled: busy,
                                    onkeydown: move |evt: KeyboardEvent| {
                                        if evt.key() == Key::Enter {
                                            enter_choice(evt, ops.busy_now(), || choose_launch_tab.call(tab), Some(launch_from_choice));
                                            return;
                                        }
                                        let target = match evt.key() {
                                            Key::ArrowLeft | Key::ArrowRight => match tab {
                                                LaunchTab::Agent => LaunchTab::Command,
                                                LaunchTab::Command => LaunchTab::Agent,
                                            },
                                            Key::Home => LaunchTab::Agent,
                                            Key::End => LaunchTab::Command,
                                            _ => return,
                                        };
                                        evt.prevent_default();
                                        if !draft.navigate_launch_tab(target, choice_targets) {
                                            return;
                                        }
                                        // After the render that moves the
                                        // tab stop, so focus lands on the tab
                                        // now selected. Only while focus is
                                        // still on a tab: a user who moved on
                                        // before that frame (Home, then Tab or
                                        // a click on Launch) keeps their control.
                                        document::eval(
                                            "requestAnimationFrame(() => { if (!document.activeElement?.matches('.create-session-form .launch-kind-tab')) return; document.querySelector('.create-session-form .launch-kind-tab.selected')?.focus(); })",
                                        );
                                    },
                                    onclick: move |_| choose_launch_tab.call(tab),
                                    "{label}"
                                }
                            }
                        }
                        }
                        if !restart_mode && launch_tab() == LaunchTab::Agent {
                        div { class: "launch-composer-choice launch-composer-harness-choice",
                            span { class: "launch-composer-section-label", "Harness" }
                            div { class: "launch-composer-options",
                                for harness in crate::launch_composer::HARNESS_PICKER_ORDER {
                                    button {
                                        r#type: "button",
                                        class: if launch_tab() == LaunchTab::Agent && *structured_harness.read() == Some(harness) { "selected" } else { "" },
                                        aria_pressed: launch_tab() == LaunchTab::Agent && *structured_harness.read() == Some(harness),
                                        "data-tooltip": "Use {crate::launch_composer::harness_label(harness)} for the session",
                                        disabled: busy,
                                        onkeydown: move |event| enter_choice(event, ops.busy_now(), || choose_harness.call(harness), Some(launch_from_choice)),
                                        onclick: move |_| choose_harness.call(harness),
                                        "{crate::launch_composer::harness_label(harness)}"
                                    }
                                }
                            }
                        }
                        }
                        if *launch_tab.read() == LaunchTab::Agent {
                            // A failed catalog read leaves the model list empty but
                            // refuses nothing (`selection_fits_catalog`), so say why the
                            // list is empty and offer to read it again rather than
                            // making the user reopen the dialog and lose its draft.
                            if let Some(message) = catalog_error.clone() {
                                // The button sits beside the live region rather than in
                                // it, so a screen reader announces the failure, not the
                                // control.
                                p { class: "create-catalog-error",
                                    span { role: "status", "Model catalog unavailable: {message} " }
                                    button {
                                        r#type: "button",
                                        class: "btn btn-neutral",
                                        "data-tooltip": "Retry: ask the host for its model list again",
                                        // Cleared first so the line goes away while the
                                        // read is out and comes back only if it fails
                                        // again; left in place, a retry would look like a
                                        // dead button until the new read settled.
                                        onclick: move |_| {
                                            launch_catalog.clear();
                                            launch_catalog.restart();
                                        },
                                        "Retry"
                                    }
                                }
                            }
                            // Semantic edits delegate live admission and retry retirement
                            // to concrete owner operations. The form retains picker UI
                            // and focus completion after those operations finish.
                            LaunchControls {
                                prompt: prompt().unwrap_or_default(),
                                on_prompt: (!restart_mode).then(|| EventHandler::new(move |value: String| { draft.edit_prompt(value); })),
                                baseline: restart_agent_baseline.clone(),
                                fixed_harness: restart_mode,
                                conversation_required,
                                conversation: conversation(),
                                on_conversation: Some(EventHandler::new(move |value| {
                                    draft.edit_conversation(value);
                                })),
                                harness: structured_harness(),
                                model: structured_model(),
                                model_raw_seed: structured_model_raw_seed(),
                                model_edited: structured_model_edited(),
                                effort: structured_effort(),
                                permissions: structured_permissions(),
                                permission_flags: structured_permission_flags(),
                                wrapper_prefix: structured_wrapper(),
                                on_wrapper_prefix: move |text| {
                                    draft.edit_wrapper(text);
                                },
                                on_permission_flags: move |text| {
                                    // A queued input can outlive the render that
                                    // enabled this editor. Keep the admitted
                                    // launch and its retry key unchanged.
                                    draft.edit_permission_flags(text);
                                },
                                catalog: catalog_models.clone(),
                                busy,
                                model_draft: model_draft(),
                                model_open: model_open(),
                                model_active: model_active(),
                                model_show_all: model_show_all(),
                                model_draft_error: model_draft_error(),
                                choice_error: structured_choice_error.map(str::to_string),
                                reset_reason: composer_reset_reason(),
                                model_id_prefix: "launch-composer-model".to_string(),
                                on_choice_enter: launch_from_choice,
                                on_model_focus: move |_| {
                                    if !draft_transition_allowed(ops) { return; }
                                    model_draft.set(String::new());
                                    model_draft_error.set(None);
                                    model_open.set(true);
                                    model_active.set(None);
                                },
                                on_model_input: move |value| {
                                    if !draft_transition_allowed(ops) { return; }
                                    model_draft.set(value);
                                    model_draft_error.set(None);
                                    model_open.set(true);
                                    model_active.set(None);
                                },
                                on_model_blur: move |_| {
                                    // A browser input can retain DOM text after a reactive render.
                                    // The selected value becomes authoritative again on blur, but
                                    // the "choose a harness" error survives so the user can follow it.
                                    model_draft.set(String::new());
                                    model_open.set(false);
                                    model_active.set(None);
                                },
                                on_model_escape: move |_| {
                                    model_draft.set(String::new());
                                    model_draft_error.set(None);
                                    model_open.set(false);
                                    model_active.set(None);
                                },
                                on_model_active: move |index| model_active.set(index),
                                on_model_enter: {
                                    let catalog_answer = catalog_answer.clone();
                                    move |target| {
                                        if !draft_transition_allowed(ops) { return; }
                                        match target {
                                            crate::launch_composer::ModelEnterTarget::Nothing => {
                                                model_draft.set(String::new());
                                                model_open.set(false);
                                                model_active.set(None);
                                            }
                                            crate::launch_composer::ModelEnterTarget::Option(option) => {
                                                apply_model_option.call(option);
                                            }
                                            crate::launch_composer::ModelEnterTarget::Custom { id: model, harness } => {
                                                draft.choose_custom_model(model, harness, catalog_answer.as_deref(),
                                                    choice_targets, model_targets);
                                            }
                                            crate::launch_composer::ModelEnterTarget::NeedsHarness(_) => {
                                                draft.report_model_harness_required(model_draft_error);
                                            }
                                            // Typing never switches a selected harness: the
                                            // draft stays for the person to correct, and the
                                            // selection is untouched.
                                            crate::launch_composer::ModelEnterTarget::OwnedElsewhere { id, owners } => {
                                                model_draft_error.set(Some(
                                                    crate::launch_composer::owned_elsewhere_message(&id, &owners),
                                                ));
                                            }
                                        }
                                    }
                                },
                                on_model_option: move |option| apply_model_option.call(option),
                                on_effort: move |effort| {
                                    // Reapplying a choice with Enter is also a
                                    // retry gesture. Only a real edit retires it.
                                    draft.choose_effort(effort, choice_targets);
                                },
                                on_permissions: move |permission| choose_permissions.call(permission),
                            }
                        }
                        if cursor_launch {
                            p { "Cursor session tracking and Resume are not supported." }
                        }
                        if *launch_tab.read() == LaunchTab::Command {
            // The raw command path: what the session runs, typed as one
            // command line.
            label { class: "launch-composer-row launch-command-invocation-field",
                span { class: "launch-composer-row-label", "Agent command" }
                input {
                    class: "font-mono",
                    r#type: "text",
                    required: retry_binding.is_none(),
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    dir: "ltr",
                    value: "{displayed_invocation}", readonly: restart_command_readonly,
                    aria_label: "Agent command",
                    disabled: busy,
                    oninput: move |evt| {
                        draft.edit_invocation(evt.value());
                    },
                }
            }
            if restart_command_readonly {
                button { r#type: "button", class: "btn btn-neutral", disabled: busy,
                    onclick: move |_| { draft.edit_invocation(String::new()); }, "Clear agent command" }
            }
            if command_changed { p { class: "launch-composer-changed-marker", "● Changed" } }
            if let Some(problem) = &restart_command_problem { p { class: "launch-command-error", role: "alert", "{problem}" } }
            // The YOLO assertion: a required choice with no default
            // (SPEC.md, Creation). Farhelm believes it and never reads the
            // command to check it.
            fieldset { class: "launch-command-yolo",
                legend { "Runs in YOLO mode" }
                label {
                    "data-tooltip": "Yes: you assert this command bypasses human, classifier and sandbox safety checks; Farhelm marks it YOLO and does not check",
                    input {
                        r#type: "radio",
                        name: "launch-command-yolo",
                        onkeydown: move |event| enter_choice(event, ops.busy_now(), || {}, Some(launch_from_choice)),
                        checked: command_yolo() == Some(true),
                        disabled: busy,
                        onchange: move |_| {
                            draft.choose_command_yolo(true);
                        },
                    }
                    "Yes (YOLO)"
                }
                label {
                    "data-tooltip": "No: you assert this command retains a safety check that may refuse or ask you; Farhelm does not check",
                    input {
                        r#type: "radio",
                        name: "launch-command-yolo",
                        onkeydown: move |event| enter_choice(event, ops.busy_now(), || {}, Some(launch_from_choice)),
                        checked: command_yolo() == Some(false),
                        disabled: busy,
                        onchange: move |_| {
                            draft.choose_command_yolo(false);
                        },
                    }
                    "No"
                }
            }
            if yolo_changed { p { class: "launch-composer-changed-marker", "● Changed" } }
            if !restart_mode {
            label { class: "launch-composer-row",
                span { class: "launch-composer-row-label", "Agent type" }
                select {
                    class: "launch-command-agent",
                    aria_label: "Agent type",
                        onkeydown: move |event| enter_choice(event, ops.busy_now(), || {}, Some(launch_from_choice)),
                    "data-tooltip": "The agent this command runs, if any, so Farhelm can track and resume it",
                    disabled: busy,
                    onchange: move |evt| {
                        draft.choose_command_agent(
                            LaunchHarness::ALL
                                .iter()
                                .copied()
                                .find(|harness| harness_value(*harness) == evt.value()),
                        );
                    },
                    option { value: "", selected: command_agent().is_none(), "None" }
                    for harness in LaunchHarness::ALL.iter().copied() {
                        option {
                            value: harness_value(harness),
                            selected: command_agent() == Some(harness),
                            {crate::launch_composer::harness_label(harness)}
                        }
                    }
                }
            }
            }
            if command_agent().is_some() {
                p { class: "launch-command-hint",
                    "Put {{farhelm_args}} in the command where Farhelm adds its own arguments."
                }
                if !restart_mode {
                label {
                    class: "launch-command-resume-choice",
                    "data-tooltip": "Resume: give the command a way to pick its conversation back up after a restart",
                    input {
                        r#type: "checkbox",
                        class: "launch-command-resume-toggle",
                        onkeydown: move |event| enter_choice(event, ops.busy_now(), || {}, Some(launch_from_choice)),
                        checked: command_resume_on(),
                        disabled: busy,
                        onchange: move |evt| {
                            draft.choose_command_resume(evt.checked());
                        },
                    }
                    "Resume"
                }
                }
                if command_resume_on() {
                    label { class: "launch-composer-row",
                        span { class: "launch-composer-row-label", "Resume command" }
                        input {
                            r#type: "text",
                            class: "launch-command-resume",
                            aria_label: "Resume command",
                            autocomplete: "off",
                            autocorrect: "off",
                            autocapitalize: "none",
                            spellcheck: "false",
                            dir: "ltr",
                            value: "{command_resume}", readonly: restart_resume_readonly,
                            disabled: busy,
                            oninput: move |evt| {
                                draft.edit_command_resume(evt.value());
                            },
                        }
                    }
                    if restart_resume_readonly {
                        button { r#type: "button", class: "btn btn-neutral", disabled: busy,
                            onclick: move |_| { draft.edit_command_resume(String::new()); }, "Clear resume command" }
                    }
                    if resume_changed { p { class: "launch-composer-changed-marker", "● Changed" } }
                }
            }
                        }
                    }
                }
            if let Some(reason) = browse_error.read().clone() {
                PeerLine {
                    class: "create-session-error".to_string(),
                    parts: vec![DetailPart::peer(reason)],
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
                    &{
                        let _ = (cwd(), cwd_edited());
                        draft.snapshot().submitted_cwd()
                    },
                )
            {
                div { class: "launch-composer-browser",
                    button {
                        r#type: "button",
                        dir: "ltr",
                        "data-tooltip": "Use this folder: the session will start here",
                        onclick: {
                            let selected_cwd = result.cwd.clone();
                            let authority = result_authority.clone();
                            move |_| {
                                draft.accept_browse_directory(selected_cwd.raw(), &authority, placement_targets,
                                    live_browse_connection, browse_activation_attempts);
                            }
                        },
                        "Use " span { class: "font-mono", "{result.cwd.escaped()}" }
                    }
                    if let Some(parent) = result.parent.clone() {
                        button {
                            r#type: "button",
                            dir: "ltr",
                            "data-tooltip": "Use the parent folder instead",
                            onclick: {
                                let authority = result_authority.clone();
                                move |_| {
                                draft.accept_browse_directory(parent.raw(), &authority, placement_targets,
                                    live_browse_connection, browse_activation_attempts);
                            }
                            },
                            "Parent: " span { class: "font-mono", "{parent.escaped()}" }
                        }
                    }
                    if result.truncated {
                        div { class: "launch-composer-browser-truncated", "More directories exist; refine the path and browse again" }
                    }
                    for child in result.children {
                        button {
                            class: "font-mono",
                            r#type: "button",
                            dir: "ltr",
                            "data-tooltip": "Use this folder: the session will start here",
                            onclick: {
                                let authority = result_authority.clone();
                                move |_| {
                                draft.accept_browse_directory(child.raw(), &authority, placement_targets,
                                    live_browse_connection, browse_activation_attempts);
                            }
                            },
                            "{child.escaped()}"
                        }
                    }
                }
            }
            if let Some((candidate, host_label)) = template_panel() {
                super::save_template::SaveTemplatePanel {
                    candidate, host_label, catalog: catalog_answer.clone(), saving: template_saving,
                    on_cancel: move |_| {
                        template_panel.set(None);
                        // The editor sits at the body's end. Returning search
                        // focus must also reveal it, without changing shared
                        // quiet-focus behavior in other modal handoffs.
                        document::eval("document.querySelector('.create-session-form[role=\"dialog\"] .launch-composer-body')?.scrollTo({top: 0, behavior: 'instant'});");
                        focus_composer_surface();
                    },
                    on_saved: on_template_saved,
                }
            }
            }
            }
            // The same form owns the pinned submit and inline editors. Long
            // action notices scroll separately so neither fields nor actions
            // are displaced by a refusal or consent explanation.
            if !loading_setup {
                DialogFooter { class: "launch-composer-footer",
                    div { class: "launch-composer-footer-row",
                        div { class: "dialog-footer-hint launch-composer-footer-summary",
                            div { class: "launch-composer-launch-context", "data-tooltip": "{target_summary}",
                                if *launch_tab.read() == LaunchTab::Command { "Command · " }
                                if let Some(harness) = &launch_harness { "{harness} · " }
                                span { class: "peer-value", dir: "ltr", "{selected_host_label}" }
                                " · "
                                span { class: "peer-value font-mono", dir: "ltr", "{summary_folder}" }
                            }
            if *launch_tab.read() == LaunchTab::Agent {
                div { class: "launch-composer-summary", aria_live: "polite",
                    "Model: "
                    span { class: "peer-value", dir: "ltr", "{summary_model}" }
                    " · Effort: {summary_effort} · Permissions: "
                    // Shared display names determine the wording; the enum
                    // determines warning styling independently of that wording.
                    if summary_permission_choice == Some(LaunchPermission::Yolo) {
                        span { class: "launch-composer-danger", "YOLO" }
                    } else {
                        "{summary_permission}"
                    }
                    if let Some(lines) = summary_prompt_lines {
                        " · Prompt: {lines} lines"
                    }
                    if container().is_some() {
                        " · retired container"
                    }
                }
            }

                        }
                        if !composer_hidden() {
                            if let Some(pending) = pending_launch() {
                                LaunchProgress { pending }
                            }
                        }
                        if let Some(snapshot) = sandbox_question() {
                            div { class: "header-confirm", role: "alertdialog", aria_label: "Create a sandbox for this session?",
                                onmounted: move |_| crate::focus_confirmation(".create-session-form [role='alertdialog'] .btn-primary", ".create-session-form [role='alertdialog'] .btn-neutral"),
                                h3 { crate::icons::SandboxMark {} "Create a sandbox for this session?" }
                                if let Some(name) = snapshot.recipe() {
                                    p { "Recipe {display_peer(&name)}" }
                                    if let Some(Ok((recipes, _))) = &sandbox_answer
                                        && let Some(recipe) = recipes.iter().find(|recipe| recipe.name == name) {
                                        p { "{recipe.size.cpus} CPU · {recipe.size.memory_gb} GB memory · {recipe.size.disk_gb} GB disk" }
                                    }
                                }
                                p { "The sandbox bills while running. Suspend preserves its processes and files and stops CPU and memory billing; snapshot storage still bills. Tensorlake suspends it after 30 minutes without a connection from Farhelm. Deleting its last session irreversibly removes the sandbox, every process, its files and snapshots." }
                                a { href: "https://www.tensorlake.ai/pricing", target: "_blank", rel: "noopener noreferrer",
                                    "data-tooltip": "Open Tensorlake's pricing page in your browser", "Tensorlake pricing" }
                                button { r#type: "button", class: "btn btn-neutral",
                                    "data-tooltip": "Cancel: create no sandbox and keep the launcher open",
                                    onclick: move |_| { sandbox_question.set(None); sandbox_consent.set(None); }, "Cancel" }
                                button { r#type: "button", class: "btn btn-primary",
                                    "data-tooltip": "Create a billed Tensorlake sandbox from this recipe and launch the session in it",
                                    onkeydown: crate::guard_confirmation_enter,
                                    onclick: move |_| {
                                        sandbox_consent.set(Some(snapshot.clone()));
                                        sandbox_question.set(None);
                                        submit_answer.call(submit_consent);
                                    }, "Create and launch" }
                            }
                        }
                        DialogActions { class: "launch-composer-actions",
                button {
                    r#type: "button",
                    class: "btn btn-neutral launch-composer-cancel",
                    aria_label: "Cancel",
                    // Keep explicit launcher cancellation separate from the
                    // isolation safety net's innermost-panel Escape target.
                    "data-template-panel-open": template_panel().is_some().then_some("true"),
                    "data-tooltip": "Cancel: close without starting anything",
                    disabled: busy || template_saving(),
                    onclick: move |_| {
                        // The disabled attribute updates after this event's
                        // synchronous submit claim. Recheck the shared lock
                        // here so a queued Cancel cannot unmount the future
                        // that owns an already accepted create.
                        if !ops.busy_now() && !template_saving() {
                            on_cancel.call(());
                        }
                    },
                    "Cancel"
                }
                if *launch_tab.read() == LaunchTab::Agent {
                    button {
                        r#type: "button",
                        class: "btn btn-neutral launch-composer-reset",
                        aria_label: "Reset choices",
                        "data-tooltip": if restart_mode { "Reset choices: restore the session's stored settings and clear the conversation selector" } else { "Reset choices: clear the agent, model and effort; remember permissions and the next agent's wrapper prefix" },
                        disabled: busy,
                        onclick: move |_| {
                            draft.reset_choices(preferences, choice_targets, composer_reset_reason,
                                composer_search, composer_search_open);
                        },
                        "Reset choices"
                    }
                }
                if !is_replace_with && recipe().is_none() && !selected.is_some_and(|id| hosts.iter().any(|host| host.id == id && host.session_owned)) {
                    button { r#type: "button", class: "btn launch-composer-save-template",
                        aria_label: "Save as template",
                        "data-tooltip": "Save as template: keep choices from this setup for another launch",
                        disabled: busy || template_saving() || template_panel().is_some(),
                        onclick: move |_| {
                            // Reopening an already mounted panel would replace
                            // its candidate while retaining its checkbox draft.
                            if !draft_transition_allowed(ops) || template_saving() || template_panel().is_some() { return; }
                            if draft.snapshot().tab() == LaunchTab::Agent
                                && let Some(reason) = draft.snapshot().container_refusal() {
                                launch_attempted.set(LaunchAttempt::None);
                                error.set(Some(reason));
                                return;
                            }
                            // The command tab retains the agent draft for a
                            // later switch, but does not save any of it.
                            if draft.snapshot().tab() == LaunchTab::Agent
                                && let Err(reason) = wrapper_words(&structured_wrapper()) {
                                // Not a launch refusal, so it must not read
                                // "Not launched."; the next launch press
                                // marks the line as one again.
                                launch_attempted.set(LaunchAttempt::None);
                                error.set(Some(reason));
                                return;
                            }
                            let state = launcher_snapshot(&draft.snapshot(), &hosts_for_template_save, selected);
                            let placement = draft.snapshot();
                            template_panel.set(Some((super::save_template::Candidate::from_state(&state,
                                super::save_template::ExplicitChoices {
                                    host: chosen_host().is_some() && clone_host_state() != CloneHostState::Bound,
                                    folder: placement.folder_is_explicit(), name: placement.title_edited(),
                                    permissions: structured_permissions_is_explicit(),
                                    wrapper: wrapper_is_explicit(),
                                }), host_label_for_template_save.clone())));
                        }, "Save as template"
                    }
                }
                button {
                    r#type: "submit",
                    class: "btn btn-primary create-session-submit",
                    "data-tooltip": launch_refusal.unwrap_or(if restart_mode {
                        if restart_stop_first { "Stop and restart: stop the working agent, then relaunch this session with these settings" }
                        else { "Restart: relaunch this session with these settings" }
                    } else if is_replace_with { "Replace: start this session, then delete the old one and its state" } else { "Launch: start the session" }),
                    aria_describedby: if restart_mode && restart_stop_first { Some(RESTART_WARNING_ID) } else { replace_warning.as_ref().map(|_| REPLACE_WARNING_ID) },
                    // Keep refused attempts reachable: native disabled would
                    // swallow both a click and implicit Enter before submit can
                    // say why. The operation guard still prevents another create.
                    aria_disabled: launch_refusal.is_some(),
                    aria_label: "{submit_verb} {target_summary}",
                    "{submit_label}"
                }

                        }
                    }
                    if show_footer_notices {
                        ScrollRegion {
                            class: "launch-composer-notices",
                            handle: COMPOSER_DIALOG, selector: ".launch-composer-notices", name: "Launch notices and confirmation",
                            recovery: RecoveryFocus::new(".create-session-submit", Some(".launch-composer-cancel"), true, RecoveryDelay::OneFrame),
            if let Some(err) = footer_error.clone() {
                // Refusals are visible beside the action they explain, even
                // when the rest of the dialog needs scrolling. Helm-owned words
                // still get the same escaping and directional isolation. A
                // refused launch is a bordered, announced alert: the plain red
                // line it used to be was easy to miss (TODO.md, 2026-10-22).
                // Only a launch press marks this slot as an attempted launch.
                // Deadline expiry keeps the alert but cannot claim refusal;
                // template errors still use the ordinary unbordered line.
                if launch_attempted().was_attempted() {
                    div { class: "create-session-error launch-composer-refusal launch-composer-refusal-alert", role: "alert",
                        span { class: "launch-composer-refusal-icon", aria_hidden: "true", "⚠" }
                        PeerLine {
                            class: "launch-composer-refusal-text".to_string(),
                            parts: vec![DetailPart::text(launch_attempted().prefix(restart_mode)), DetailPart::peer(err)],
                        }
                    }
                } else {
                    PeerLine {
                        class: "create-session-error launch-composer-refusal".to_string(),
                        parts: vec![DetailPart::peer(err)],
                    }
                }
            }
            // These warnings stay beside the pinned action: pressing it is
            // the confirmation, so the warning must remain in that viewport.
            // Polite live region: the warning can appear or change while the
            // launcher stays open, and the replace button names it as its
            // description, since pressing that button is the confirmation.
            if restart_mode && restart_stop_first {
                p { id: RESTART_WARNING_ID, class: "launch-composer-restart-warning", aria_live: "polite", "The agent is working. This action stops it before restarting." }
            }
            if is_replace_with {
                div { id: REPLACE_WARNING_ID, class: "launch-composer-replace-warning", aria_live: "polite",
                    if let Some(warning) = &replace_warning {
                        "{warning}"
                    }
                }
            }
            // Keep the question adjacent to the action that raised it. Its
            // bounded scroll region can reveal every answer without letting a
            // long explanation displace the action row or the editable body.
            //
            // Shown only while the draft still holds the refused request's
            // key: an edit that changes what would launch retires the key,
            // and with it a question that no longer describes the draft.
            // Templates retain the whole pair for submit's comparison, so
            // their completed acceptance separately cancels a changed question.
            if let Some(question) = yolo_refusal.current().filter(|_| !composer_hidden())
                && let Some((_, refused_binding)) = intent_key.read().clone().filter(|(key, _)| *key == question.key().refused_key)
            {
                crate::yolo_confirm::YoloConfirmation {
                    ask: question.key().ask.clone(),
                    may_have_started: question.key().uncertain,
                    busy,
                    error: yolo_error(),
                    // Both YOLO answers submit the form themselves, and only
                    // once they have taken this opening of the question
                    // (`answer_launcher_yolo`). As native submit buttons they
                    // would submit it whether or not the question was still
                    // open, sending a cancelled draft again.
                    confirm_submits: false,
                    // The one-off also disarms any "don't ask again"
                    // request (`answer_launcher_yolo`): it must never mark
                    // the host.
                    on_launch_guarded: {
                        let question = question.clone();
                        let refused_binding = refused_binding.clone();
                        let selection = match &refused_binding.agent {
                            LaunchIntent::Structured(selection) => Some(selection),
                            LaunchIntent::Command(_) => None,
                        };
                        // What this render shows: the launcher opening the
                        // question sits in, and the Replace warning above it.
                        // The answer binds to both (`GuardedSubmitStamp`).
                        let rendered_opening = *opening_generation.peek();
                        (question.key().offers_guarded(selection) && draft.snapshot().restart_guarded_changes()).then(|| Callback::new(move |_| {
                            if !draft.snapshot().restart_guarded_changes() { return; }
                            let held = intent_key.peek().clone();
                            let current = held.as_ref()
                                .filter(|(key, _)| key == &question.key().refused_key)
                                .and_then(|_| live_yolo_binding());
                            if !take_launcher_guarded_answer(
                                &mut yolo_refusal, &question, ops.busy_now(),
                                rendered_opening, *opening_generation.peek(),
                                &refused_binding, current.as_ref(),
                            ) { return; }
                            let press_baseline = launch_activity.snapshot();
                            yolo_error.set(None);
                            yolo_confirmed_key.set(None);
                            yolo_stop_asking.set(None);
                            choose_permissions.call(Some(LaunchPermission::Guarded));
                            if draft.snapshot().destination().repo().is_some() {
                                // The row transition changes the fingerprint
                                // before render installs its new authority.
                                // Carry the remaining accepted authority into
                                // that expected refresh, without reusing its
                                // preview or the refused YOLO request body.
                                if let Some(mut authority) = live_preview_authority.peek().clone() {
                                    authority.agent = preview_agent_now();
                                    guarded_submit_pending.set(Some((GuardedSubmitStamp::new(
                                        rendered_opening, authority,
                                        draft.snapshot().prompt(), draft.snapshot().conversation(),
                                        replace_guard,
                                        press_baseline,
                                    ), None)));
                                }
                            } else {
                                submit_composer.call((submit_consent.0, submit_consent.1, submit_consent.2, press_baseline, None));
                            }
                        }))
                    },
                    // Both "start anyway" answers submit with this render's
                    // `replace_guard`, the precondition matching the Replace
                    // warning drawn above the question, by calling
                    // `submit_composer` directly. Going through the browser
                    // (`requestSubmit`) would run `onsubmit` as a later
                    // event with a later render's precondition, which may
                    // authorize deleting more than the warning the user
                    // answered said. Nothing is lost by skipping it: the
                    // form is `novalidate`, so the browser validated nothing
                    // there, and `submit_composer` keeps every refusal of
                    // its own and still reads the live draft, whose intent
                    // key decides whether this consent applies to what is
                    // sent. The answer also refuses once the launcher has
                    // moved to another opening (`answer_launcher_yolo`), so
                    // it never acts on a source the user has moved away
                    // from.
                    //
                    // The `write()` guards are temporaries of the `if`
                    // condition, dropped before the block runs, which
                    // matters because `submit_composer` writes
                    // `yolo_stop_asking` itself.
                    on_confirm: {
                        let question = question.clone();
                        let rendered_opening = *opening_generation.peek();
                        move |_| {
                            if answer_launcher_yolo(
                                &mut yolo_refusal,
                                &question,
                                false,
                                rendered_opening,
                                *opening_generation.peek(),
                                &mut yolo_confirmed_key.write(),
                                &mut yolo_stop_asking.write(),
                            ) {
                                submit_answer.call(submit_consent);
                            }
                        }
                    },
                    on_confirm_and_stop_asking: {
                        let question = question.clone();
                        let rendered_opening = *opening_generation.peek();
                        move |_| {
                            if answer_launcher_yolo(
                                &mut yolo_refusal,
                                &question,
                                true,
                                rendered_opening,
                                *opening_generation.peek(),
                                &mut yolo_confirmed_key.write(),
                                &mut yolo_stop_asking.write(),
                            ) {
                                submit_answer.call(submit_consent);
                            }
                        }
                    },
                    on_cancel: move |_| {
                        // Only this opening: a cancel drawn for an earlier
                        // question leaves a later one (and its error) alone.
                        if yolo_refusal.take(&question).is_some() {
                            if restart_mode { document::eval("document.querySelector('.create-session-form .launch-composer-cancel')?.focus({preventScroll:true})"); }
                            // Explicit Cancel owes a focus handoff. Template
                            // edits that retire a question keep their own caret.
                            else { focus_composer_surface(); }
                            yolo_error.set(None);
                        }
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

#[cfg(test)]
mod tests {
    /// A newly created sandbox can precede its host metadata in the browser.
    /// Neither New nor a queued shortcut may use local placement in that gap;
    /// an explicit replacement target is independent of the unresolved source.
    #[farhelm_testtrace::test]
    fn missing_creation_origin_requires_metadata_or_an_explicit_target() {
        assert!(super::unresolved_creation_origin(Some(7), &[], None, None));
        assert!(super::unresolved_creation_origin(
            Some(7),
            &[],
            Some(7),
            None
        ));
        assert!(!super::unresolved_creation_origin(
            Some(7),
            &[],
            Some(1),
            None
        ));
        assert!(!super::unresolved_creation_origin(
            Some(7),
            &[],
            None,
            Some("dev")
        ));
        assert!(!super::unresolved_creation_origin(None, &[], None, None));
    }

    /// The real owner and question hooks let completion be tested before any
    /// renderer memo settles. This fixture retains the actual template targets,
    /// not a parallel draft, retry comparison or confirmation implementation.
    #[derive(Clone, Copy)]
    struct TemplateQuestionFixture {
        owner: super::LaunchDraftState,
        targets: super::SearchTargets,
        preferences: super::SharedPreferences,
        key: dioxus::prelude::Signal<Option<(String, super::IntentBinding)>>,
        slot: crate::ops::ConfirmSlot<super::LauncherYoloQuestion>,
    }
    std::thread_local! {
        static TEMPLATE_QUESTION_FIXTURE: std::cell::Cell<Option<TemplateQuestionFixture>> = const { std::cell::Cell::new(None) };
    }

    /// Mount the same owner/key/host and actual replay dependencies. Each tab
    /// case mounts fresh; no arbitrary snapshot installation establishes it.
    fn template_question_fixture() -> dioxus::prelude::Element {
        use super::*;
        let ops = crate::ops::use_op_lock();
        let key = use_signal(|| None);
        let chosen_host = use_signal(|| Some(1));
        let owner = use_launch_draft(None, None, ops, key, chosen_host);
        let choices = ChoiceTargets {
            partial_template_hint: use_signal(|| false),
            offered_history: use_signal(|| None),
            create_target: use_signal(|| None),
            fetched_history: use_signal(|| None),
            offered_environment_history: use_signal(|| None),
            fetched_environment_history: use_signal(|| None),
        };
        let clone_host_state = use_signal(|| CloneHostState::Unconfirmable);
        let remembered_destination = use_signal(|| None);
        let recovery = RecoveryTargets {
            composer_reset_reason: use_signal(|| None),
            clone_host_state,
            remembered_destination,
            model_draft: use_signal(String::new),
            model_draft_error: use_signal(|| None),
        };
        let placement = PlacementTargets {
            choices,
            clone_host_state,
            remembered_destination,
            live_destination: use_signal(|| None),
            history_activation_attempts: use_signal(|| 0),
            preview_revision: use_signal(|| 0),
            browse_generation: use_signal(|| 0),
            browse_request: use_signal(|| None),
            browse_result: use_signal(|| None),
            browse_error: use_signal(|| None),
        };
        let targets = SearchTargets {
            placement,
            recovery,
            template_error: use_signal(|| None),
        };
        let preferences = SharedPreferences(use_signal(api::Preferences::default));
        let slot = use_confirm_slot();
        TEMPLATE_QUESTION_FIXTURE.with(|value| {
            value.set(Some(TemplateQuestionFixture {
                owner,
                targets,
                preferences,
                key,
                slot,
            }))
        });
        rsx! {}
    }

    /// Run actual validated owner replay; the assertions below exercise the
    /// existing completed-question functions immediately after publication.
    fn replay_template_question_sets(
        fixture: TemplateQuestionFixture,
        hosts: &[super::HostOption],
        fields: farhelm_proto::launcher::TemplateFields,
    ) {
        use super::*;
        let template = farhelm_proto::launcher::LaunchTemplate {
            name: "fixture".into(),
            fields,
        };
        assert!(
            fixture
                .owner
                .accept_search_result(
                    crate::launch_composer::ComposerSearchResult::Template("fixture".into()),
                    Some(&template),
                    false,
                    SearchInputs {
                        recipes: &[],
                        templates: &[],
                        hosts,
                        catalog: Some(&[]),
                        host_fixed: false,
                        history_target: None,
                        preferences: fixture.preferences
                    },
                    fixture.targets
                )
                .is_some()
        );
    }

    /// An unchanged completed Agent or Command retains the actual opening and
    /// whole retry pair. A changed completed template retires the slot before
    /// render, so restoring old choices cannot revive it or authorize a queued
    /// answer. Dormant malformed Agent text must not invalidate a Command.
    #[farhelm_testtrace::test]
    fn completed_templates_retire_only_changed_questions_before_render() {
        use super::*;
        use farhelm_proto::launcher::{LauncherKind, TemplateFields};
        let hosts = [option(1, "local", true)];
        let target = CreateTarget::new(1, hosts[0].incarnation.clone());
        for tab in [LaunchTab::Agent, LaunchTab::Command] {
            let mut dom = VirtualDom::new(template_question_fixture);
            dom.rebuild_in_place();
            let mut fixture = TEMPLATE_QUESTION_FIXTURE
                .with(std::cell::Cell::get)
                .unwrap();
            dom.in_runtime(|| {
                assert!(
                    fixture
                        .owner
                        .choose_harness(
                            LaunchHarness::Codex,
                            None,
                            fixture.preferences,
                            fixture.targets.placement.choices,
                            fixture.targets.recovery
                        )
                        .is_some()
                );
                fixture.owner.choose_permissions(
                    Some(LaunchPermission::Yolo),
                    fixture.targets.placement.choices,
                );
                fixture
                    .owner
                    .choose_launch_tab(tab, fixture.targets.placement.choices);
                fixture.owner.edit_invocation("safe-command".into());
                fixture.owner.choose_command_yolo(true);
                if tab == LaunchTab::Command {
                    fixture
                        .owner
                        .edit_wrapper("'unfinished dormant wrapper".into());
                    fixture
                        .owner
                        .edit_permission_flags("'unfinished dormant flags".into());
                }
                let before = fixture.owner.snapshot();
                let refused = before
                    .request(RequestContext {
                        host: Some((1, &hosts[0].incarnation)),
                        launch: before.launch_intent().unwrap(),
                        replace_source: None,
                        checkout: None,
                    })
                    .unwrap();
                let retained = ("refused-key".to_string(), refused.clone());
                fixture.key.set(Some(retained.clone()));
                let asked = LauncherYoloQuestion {
                    refused_key: retained.0.clone(),
                    ask: crate::yolo_confirm::YoloAsk {
                        host: Some(1),
                        host_name: "local".into(),
                        reason: if tab == LaunchTab::Agent {
                            crate::yolo_confirm::YoloReason::ChosenPermission
                        } else {
                            crate::yolo_confirm::YoloReason::Asserted
                        },
                    },
                    uncertain: false,
                };
                let question = fixture.slot.open(asked.clone(), ());
                let unchanged = if tab == LaunchTab::Agent {
                    TemplateFields {
                        model: Some(None),
                        ..Default::default()
                    }
                } else {
                    TemplateFields {
                        kind: Some(LauncherKind::Command),
                        command: Some("safe-command".into()),
                        yolo: Some(true),
                        ..Default::default()
                    }
                };
                replay_template_question_sets(fixture, &hosts, unchanged);
                assert!(fixture.targets.template_error.peek().is_none());
                assert_eq!(*fixture.key.peek(), Some(retained.clone()));
                let current = completed_template_question_binding(
                    &fixture.owner.snapshot(),
                    &hosts,
                    Some(1),
                    Some(&target),
                    None,
                    &refused,
                );
                assert_eq!(current.as_ref(), Some(&refused));
                assert!(!retire_changed_template_question(
                    &mut fixture.slot,
                    Some(&retained),
                    current.as_ref()
                ));
                assert!(fixture.slot.peek_token() == Some(question.clone()));

                // A rejected template runs no semantic replay and has no successful
                // completion. It must not alter this mounted question or pair.
                let snapshot = fixture.owner.snapshot();
                replay_template_question_sets(
                    fixture,
                    &hosts,
                    TemplateFields {
                        kind: Some(LauncherKind::Command),
                        model: Some(Some("inapplicable".into())),
                        ..Default::default()
                    },
                );
                assert!(fixture.targets.template_error.peek().is_some());
                assert_eq!(fixture.owner.snapshot(), snapshot);
                assert!(fixture.slot.peek_token() == Some(question.clone()));
                assert_eq!(*fixture.key.peek(), Some(retained.clone()));

                let changed = if tab == LaunchTab::Agent {
                    TemplateFields {
                        permissions: Some(Some(LaunchPermission::Guarded)),
                        ..Default::default()
                    }
                } else {
                    TemplateFields {
                        command: Some("different-command".into()),
                        ..Default::default()
                    }
                };
                replay_template_question_sets(fixture, &hosts, changed);
                assert!(fixture.targets.template_error.peek().is_none());
                let current = completed_template_question_binding(
                    &fixture.owner.snapshot(),
                    &hosts,
                    Some(1),
                    Some(&target),
                    None,
                    &refused,
                );
                assert_ne!(current.as_ref(), Some(&refused));
                assert!(retire_changed_template_question(
                    &mut fixture.slot,
                    Some(&retained),
                    current.as_ref()
                ));
                assert!(
                    fixture.slot.peek_key().is_none(),
                    "the real slot is empty before any render"
                );
                assert_eq!(
                    *fixture.key.peek(),
                    Some(retained.clone()),
                    "template completion preserves submit's whole pair"
                );

                replay_template_question_sets(
                    fixture,
                    &hosts,
                    if tab == LaunchTab::Agent {
                        TemplateFields {
                            permissions: Some(Some(LaunchPermission::Yolo)),
                            ..Default::default()
                        }
                    } else {
                        TemplateFields {
                            command: Some("safe-command".into()),
                            ..Default::default()
                        }
                    },
                );
                let restored = completed_template_question_binding(
                    &fixture.owner.snapshot(),
                    &hosts,
                    Some(1),
                    Some(&target),
                    None,
                    &refused,
                );
                assert_eq!(restored.as_ref(), Some(&refused));
                assert!(!retire_changed_template_question(
                    &mut fixture.slot,
                    Some(&retained),
                    restored.as_ref()
                ));
                assert!(
                    fixture.slot.peek_key().is_none(),
                    "restoration cannot resurrect the opening"
                );
                let (mut consent, mut stop) = (None, None);
                for permanent in [false, true] {
                    assert!(!answer_launcher_yolo(
                        &mut fixture.slot,
                        &question,
                        permanent,
                        1,
                        1,
                        &mut consent,
                        &mut stop
                    ));
                }
                assert!(!take_launcher_guarded_answer(
                    &mut fixture.slot,
                    &question,
                    false,
                    1,
                    1,
                    &refused,
                    Some(&refused)
                ));
                assert!(consent.is_none() && stop.is_none());
                // The same refused request asked again: an equal key, a
                // later opening the old answers must not reach.
                let later = fixture.slot.open(asked.clone(), ());
                fixture.slot.cancel_for(&question);
                assert!(!answer_launcher_yolo(
                    &mut fixture.slot,
                    &question,
                    true,
                    1,
                    1,
                    &mut consent,
                    &mut stop
                ));
                assert!(!take_launcher_guarded_answer(
                    &mut fixture.slot,
                    &question,
                    false,
                    1,
                    1,
                    &refused,
                    Some(&refused)
                ));
                assert!(fixture.slot.peek_token() == Some(later.clone()));
                assert!(answer_launcher_yolo(
                    &mut fixture.slot,
                    &later,
                    false,
                    1,
                    1,
                    &mut consent,
                    &mut stop
                ));
                assert_eq!(consent.as_deref(), Some("refused-key"));
                fixture.slot.open(asked.clone(), ());
                let other_pair = ("another-key".into(), refused.clone());
                assert!(retire_changed_template_question(
                    &mut fixture.slot,
                    Some(&other_pair),
                    Some(&refused)
                ));
                assert!(
                    fixture.slot.peek_key().is_none(),
                    "binding equality alone cannot keep a different key's opening"
                );
            });
        }
    }

    /// A managed template's identity uses its refused accepted checkout, not a
    /// new preview offer or dormant candidate cwd. Pending preview observation
    /// therefore cannot close an unchanged question; actual request/host changes do.
    #[farhelm_testtrace::test]
    fn completed_managed_template_identity_keeps_accepted_checkout_facts() {
        use super::*;
        let hosts = [option(1, "local", true)];
        let target = CreateTarget::new(1, hosts[0].incarnation.clone());
        let repo = GithubRepo::parse("acme/repo").unwrap();
        let mut draft = LaunchDraft::new(Some(LaunchPermission::Yolo));
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        draft.edit_title("work".into());
        draft.select_repository(repo.clone());
        let preview = crate::github_checkout::GithubPreview {
            canonical_root: "/accepted".into(),
            basename: "work".into(),
            cwd: "/accepted/work".into(),
            config_revision: 9,
            host: "1".into(),
            incarnation: 1,
            installation_identity: "install-1".into(),
        };
        let refused = draft
            .request(RequestContext {
                host: Some((1, &hosts[0].incarnation)),
                launch: draft.launch_intent().unwrap(),
                replace_source: None,
                checkout: Some(CheckoutInput::Preview {
                    repo: &repo,
                    preview: &preview,
                }),
            })
            .unwrap();
        let current = completed_template_question_binding(
            &draft,
            &hosts,
            Some(1),
            Some(&target),
            None,
            &refused,
        )
        .unwrap();
        assert_eq!(
            current, refused,
            "accepted cwd/preview/title survive without any Ready observation"
        );
        draft.edit_title("different".into());
        assert!(
            completed_template_question_binding(
                &draft,
                &hosts,
                Some(1),
                Some(&target),
                None,
                &refused
            )
            .is_none()
        );
        draft.edit_title("work".into());
        let replaced = [HostOption {
            incarnation: "replaced".into(),
            identity: Some("other-install".into()),
            ..hosts[0].clone()
        }];
        let replaced_target = CreateTarget::new(1, "replaced".into());
        assert!(
            completed_template_question_binding(
                &draft,
                &replaced,
                Some(1),
                Some(&replaced_target),
                None,
                &refused
            )
            .is_none()
        );
        assert!(
            completed_template_question_binding(
                &draft,
                &hosts,
                Some(1),
                Some(&replaced_target),
                None,
                &refused
            )
            .is_none()
        );
    }

    /// An unfinished custom editor can also fail catalog compatibility. Its
    /// actionable repair must win over the unrelated saved-model explanation.
    #[test]
    fn custom_flag_refusal_precedes_catalog_incompatibility() {
        let mut prerequisites = super::LaunchPrerequisites {
            busy: false,
            saving_template: false,
            host_available: true,
            destination_ready: true,
            github_search: false,
            remembered_destination_valid: true,
            folder_default_waiting: false,
            needs_harness: false,
            invalid_permission_flags: true,
            incompatible_choice: true,
            invalid_wrapper: false,
        };
        assert_eq!(
            prerequisites.refusal(),
            Some(super::CUSTOM_PERMISSION_FLAGS_REFUSAL)
        );
        prerequisites.invalid_permission_flags = false;
        assert!(prerequisites.refusal().unwrap().contains("current catalog"));
    }

    /// Inherited wrapper words seed the same shell draft, including quoting and
    /// unsafe characters. Keep the raw draft available for the control's existing
    /// inherited-value guard, separately from its visible escape spelling.
    #[test]
    fn remembered_wrapper_joins_original_shell_words() {
        let preferences: super::api::Preferences = serde_json::from_value(serde_json::json!({
            "remembered_wrapper_prefixes": {"codex": ["wrapper\u{202e}", "'argument with spaces'", "''"]}
        })).unwrap();
        let draft = super::remembered_wrapper(&preferences, Some(super::LaunchHarness::Codex));
        assert_eq!(draft, "wrapper\u{202e} 'argument with spaces' ''");
        assert!(crate::peer::editable_peer(&draft).contains("wrapper<U+202E>"));
    }

    /// A remembered prefix belongs to one agent type. Editing invalid shell
    /// text must stay visible and refuse launch instead of submitting none.
    #[test]
    fn wrapper_draft_parses_and_reads_only_its_agent_memory() {
        use super::*;
        let mut preferences = api::Preferences::default();
        preferences
            .remembered_wrapper_prefixes
            .insert(LaunchHarness::Codex, vec!["wrapper".into(), "--".into()]);
        assert_eq!(
            remembered_wrapper(&preferences, Some(LaunchHarness::Codex)),
            "wrapper --"
        );
        assert_eq!(
            remembered_wrapper(&preferences, Some(LaunchHarness::Claude)),
            ""
        );
        assert_eq!(wrapper_words(""), Ok(None));
        assert_eq!(
            wrapper_words("wrapper --"),
            Ok(Some(vec!["wrapper".into(), "--".into()]))
        );
        assert!(wrapper_words("'unfinished").is_err());
    }

    /// A quick origin is insufficient after New became visible. A successful
    /// retry after refusal or consent has already offered the name field and
    /// must not be followed by a second naming prompt.
    #[test]
    fn naming_fact_requires_a_quick_opening_that_never_revealed_new() {
        assert!(super::new_was_never_shown(true, true));
        assert!(!super::new_was_never_shown(true, false));
        assert!(!super::new_was_never_shown(false, false));
        assert!(!super::new_was_never_shown(false, true));
    }

    /// A hidden prompt remains an agent-tab draft, never a command argument.
    /// Empty presence survives templates; unsupported agents omit it.
    #[test]
    fn initial_prompt_projection_keeps_only_supported_agent_requests() {
        use super::*;
        use farhelm_proto::launcher::LauncherKind;
        let message = Some("--literal\nsecond".to_string());
        for harness in LaunchHarness::ALL {
            assert_eq!(
                active_prompt(LauncherKind::Agent, Some(*harness), &message),
                harness
                    .supports_initial_prompt(false)
                    .then(|| message.clone())
                    .flatten()
            );
            assert_eq!(
                active_prompt(LauncherKind::Command, Some(*harness), &message),
                None
            );
        }
        assert_eq!(active_prompt(LauncherKind::Agent, None, &message), message);
        assert_eq!(
            active_prompt(
                LauncherKind::Agent,
                Some(LaunchHarness::Codex),
                &Some(String::new())
            ),
            Some(String::new())
        );
    }
    /// Seeds must show history rather than filtering by a clone path or `~`.
    /// Typed text filters before the eight-row cap, preserving newest-first order.
    #[test]
    fn folder_dropdown_filters_only_typed_text_and_caps_matches() {
        let folders = (0..12)
            .map(|index| super::api::FolderHistoryEntry {
                host: 1,
                canonical_cwd: format!("/work/{index}").into(),
                canonical_proven: true,
                display_cwd: format!(
                    "/work/{}-{index}",
                    if index % 2 == 0 { "MATCH" } else { "other" }
                )
                .into(),
                created_at: 12 - index,
                creation_seq: Some((12 - index) as u64),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            super::folder_suggestions(&folders, "~", false),
            folders[..8].iter().collect::<Vec<_>>()
        );
        assert_eq!(
            super::folder_suggestions(&folders, "/cloned/path", false).len(),
            8
        );
        assert_eq!(super::folder_suggestions(&folders, "work", true).len(), 8);
        assert_eq!(
            super::folder_suggestions(&folders, "mAtCh", true),
            folders.iter().step_by(2).collect::<Vec<_>>()
        );
        assert!(super::folder_suggestions(&folders, "missing", true).is_empty());
    }

    /// Shared-prefix siblings must remain distinguishable, including non-ASCII
    /// names. Peer control characters still pass through the display escaping.
    #[test]
    fn folder_dropdown_labels_preserve_the_escaped_tail() {
        assert_eq!(super::folder_tail_label(&"~/short".into()), "~/short");
        let at_limit = "123456789012345678901234567890123456";
        assert_eq!(super::folder_tail_label(&at_limit.into()), at_limit);
        let long = format!("/{}終わり", "界".repeat(40));
        let label = super::folder_tail_label(&long.into());
        assert_eq!(
            label,
            concat!(
                "…界界界界界界界界",
                "界界界界界界界界",
                "界界界界界界界界",
                "界界界界界界界界終わり"
            )
        );
        assert_eq!(
            super::folder_tail_label(&"/tmp/\nname".into()),
            "/tmp/<U+000A>name"
        );
    }

    /// A character-safe tail can still leave `02E>` from an escaped direction
    /// override. Drop the whole crossing marker so it cannot look like ordinary
    /// path text. Literal marker text earlier in the path must not shift the
    /// generated token's displayed position or acquire escape semantics itself.
    #[test]
    fn folder_dropdown_labels_drop_a_split_escape_marker() {
        for marker in ["\u{202E}", "\u{E0041}"] {
            let path =
                format!("~/界界界界界界界界界界/<U+200B>/{marker}abcdefghijklmnopqrstuvwxyz12345");
            assert_eq!(
                super::folder_tail_label(&path.into()),
                "…abcdefghijklmnopqrstuvwxyz12345",
                "a crossing marker must be omitted whole: {marker:?}"
            );
        }
    }

    /// Printable marker-shaped names need no escape and keep the existing tail.
    /// A real unsafe character in the omitted prefix prevents a whole-input
    /// unsafe check from accidentally giving the literal marker token semantics.
    #[test]
    fn folder_dropdown_labels_keep_literal_marker_text_as_ordinary_path_characters() {
        for (marker, expected) in [
            ("<U+202E>", "…02E>abcdefghijklmnopqrstuvwxyz12345"),
            ("<U+E0041>", "…041>abcdefghijklmnopqrstuvwxyz12345"),
            ("<U+10FFFF>", "…FFF>abcdefghijklmnopqrstuvwxyz12345"),
        ] {
            let path =
                format!("~/界界界界界界界界界界/\u{200b}/{marker}abcdefghijklmnopqrstuvwxyz12345");
            assert_eq!(super::folder_tail_label(&path.into()), expected);
        }
    }

    /// A complete marker that fits in the tail still exposes the character it
    /// represents. Exact boundaries must not discard it or consume the final
    /// path characters to retain it; six-digit markers can occur as literal text.
    #[test]
    fn folder_dropdown_labels_keep_complete_markers_at_the_boundary() {
        for (tail, expected) in [
            (
                "\u{202E}abcdefghijklmnopqrstuvwxyz1",
                "…<U+202E>abcdefghijklmnopqrstuvwxyz1",
            ),
            (
                "\u{E0041}abcdefghijklmnopqrstuvwxyz",
                "…<U+E0041>abcdefghijklmnopqrstuvwxyz",
            ),
            (
                "<U+10FFFF>abcdefghijklmnopqrstuvwxy",
                "…<U+10FFFF>abcdefghijklmnopqrstuvwxy",
            ),
            (
                "\u{202E}abcdefghijklmnopqrstuvwxyz123456789",
                "…abcdefghijklmnopqrstuvwxyz123456789",
            ),
        ] {
            let path = format!("~/界界界界界界界界界界/{tail}");
            assert_eq!(super::folder_tail_label(&path.into()), expected);
        }
    }

    /// A command launch with only its command, asserted not YOLO: the
    /// command-mode intent these tests vary.
    fn test_command(command: &str) -> super::CommandLaunch {
        super::CommandLaunch {
            command: command.to_string(),
            yolo: false,
            agent: None,
            resume: None,
        }
    }

    /// The banner orders approval mechanisms by policy, not by enum order or
    /// agent identity. Own-setting and custom flags have no safety guarantee;
    /// equivalent safety groups must not create spurious escalations.
    #[test]
    fn permission_warning_orders_modes_by_their_safety_group() {
        use crate::LaunchPermission::*;
        assert_eq!(
            permission_danger(Some(Chat)),
            permission_danger(Some(Approve))
        );
        assert_eq!(
            permission_danger(Some(SmartApprove)),
            permission_danger(Some(Guarded))
        );
        assert_eq!(permission_danger(None), permission_danger(Some(Custom)));
        assert_eq!(
            permission_danger(Some(Custom)),
            permission_danger(Some(Yolo))
        );
        assert!(permission_danger(Some(Approve)) < permission_danger(Some(Guarded)));
        assert!(permission_danger(Some(Guarded)) < permission_danger(Some(Yolo)));
    }

    /// The helm's remembered permissions word seeds the composer's segment
    /// only when it is a mode this build knows: `"yolo"` preselects yolo,
    /// nothing remembered preselects default, and a word a newer helm might
    /// store reads as default rather than as an error — the same tolerance
    /// the list-order preference has, for the same reason (the row outlives
    /// the build that validated it).
    #[test]
    fn the_remembered_permissions_word_seeds_only_a_mode_this_build_knows() {
        let with = |word: Option<&str>| super::api::Preferences {
            last_container_host: None,
            auto_update_remote_hosts: None,
            visit_history: Vec::new(),
            open_extensions: None,
            list_sort: None,
            last_selected: None,
            compact: None,
            remembered_permissions: word.map(str::to_string),
            remembered_wrapper_prefixes: Default::default(),
            remembered_workspace_trust: None,
            skip_host_remove_confirmation: None,
            skip_host_setup_confirmation: None,
            feedback_contact: None,
            terminal_link_click: None,
            skip_session_confirmations: None,
            name_session_on_quick_launch: None,
            color_terminal_frames: None,
            quick_launch_template: None,
            template_shortcuts: Default::default(),
            welcome_seen: None,
            claude_trust_hint_dismissed: None,
            codex_trust_hint_dismissed: None,
            muse_trust_hint_dismissed: None,
            pi_trust_hint_dismissed: None,
            tunnel_consent: None,
        };
        assert_eq!(
            super::initial_structured_permissions(&with(Some("yolo"))),
            Some(super::LaunchPermission::Yolo)
        );
        assert_eq!(
            super::initial_structured_permissions(&with(Some("approve"))),
            Some(super::LaunchPermission::Approve)
        );
        assert_eq!(
            super::initial_structured_permissions(&with(Some("smart_approve"))),
            Some(super::LaunchPermission::SmartApprove)
        );
        assert_eq!(
            super::initial_structured_permissions(&with(Some("chat"))),
            Some(super::LaunchPermission::Chat)
        );
        assert_eq!(super::initial_structured_permissions(&with(None)), None);
        assert_eq!(
            super::initial_structured_permissions(&with(Some("supervised"))),
            None,
            "a word this build does not know must read as nothing remembered"
        );
    }

    use super::super::row::row_specimen;
    use super::super::shared::tests::{open, option};
    use super::*;

    /// A lost fresh-create reply remains reconcilable after reconnect and a
    /// changed preview, but cannot be replayed for another installation, agent,
    /// repo, title or replacement source. Ordinary bindings never qualify.
    #[test]
    fn fresh_retry_matches_user_intent_independently_of_current_preview() {
        let host = option(1, "target", true);
        let mut draft = LaunchDraft::new(None);
        draft.edit_folder("/old/bar-1".into());
        draft.edit_title("work".into());
        let mut original = draft
            .request(RequestContext {
                host: Some((host.id, &host.incarnation)),
                launch: LaunchIntent::Command(test_command("agent-a")),
                replace_source: Some("source-a"),
                checkout: None,
            })
            .unwrap();
        let repo = GithubRepo::parse("acme/bar").unwrap();
        original.github_checkout = Some(GithubCheckoutRequest {
            repo: repo.identifier(),
            title: Some("work".into()),
            preview: crate::github_checkout::GithubPreview {
                canonical_root: "/old".into(),
                basename: "bar-1".into(),
                cwd: "/old/bar-1".into(),
                config_revision: 1,
                host: "1".into(),
                incarnation: 1,
                installation_identity: "install-a".into(),
            },
        });
        let mut current = original.clone();
        for ignored in [
            IntentBinding {
                cwd: "/candidate/path".into(),
                ..original.clone()
            },
            IntentBinding {
                incarnation: "candidate-incarnation".into(),
                ..original.clone()
            },
            IntentBinding {
                conversation: Some("candidate-selector".into()),
                ..original.clone()
            },
            IntentBinding {
                github_checkout: None,
                ..original.clone()
            },
        ] {
            assert!(same_fresh_intent(&original, &ignored, &repo, "install-a"));
            assert_ne!(
                original, ignored,
                "ordinary key equality still includes the ignored candidate member"
            );
        }
        current.incarnation = "reconnected".into();
        current.cwd = "/new/bar-2".into();
        current.github_checkout = None;
        assert!(same_fresh_intent(&original, &current, &repo, "install-a"));
        // A render candidate is discarded after matching. These members never
        // determine whether the retained original binding/body is selected.
        let mut ignored = current.clone();
        ignored.conversation = Some("candidate-only selector".into());
        assert!(same_fresh_intent(&original, &ignored, &repo, "install-a"));
        let mut ignored = current.clone();
        let mut different_checkout = original.github_checkout.clone().unwrap();
        different_checkout.repo = "unrelated/candidate".into();
        different_checkout.title = Some("candidate checkout title".into());
        different_checkout.preview.config_revision = 99;
        different_checkout.preview.installation_identity = "candidate-install".into();
        ignored.github_checkout = Some(different_checkout);
        assert!(same_fresh_intent(&original, &ignored, &repo, "install-a"));
        assert!(!same_fresh_intent(&current, &current, &repo, "install-a"));
        assert!(!same_fresh_intent(&original, &current, &repo, "install-b"));
        assert!(!same_fresh_intent(
            &original,
            &current,
            &GithubRepo::parse("acme/other").unwrap(),
            "install-a"
        ));
        let mut changed = current.clone();
        changed.agent = LaunchIntent::Command(test_command("agent"));
        assert!(!same_fresh_intent(&original, &changed, &repo, "install-a"));
        let mut changed = current.clone();
        changed.title = "different".into();
        assert!(!same_fresh_intent(&original, &changed, &repo, "install-a"));
        let mut changed = current.clone();
        changed.prompt = Some("different first message".into());
        assert!(!same_fresh_intent(&original, &changed, &repo, "install-a"));
        let mut changed = current.clone();
        changed.replace_source = Some("source-b".into());
        assert!(!same_fresh_intent(&original, &changed, &repo, "install-a"));
        current.host = 2;
        assert!(!same_fresh_intent(&original, &current, &repo, "install-a"));
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
    /// case of the same rule: a structured launch and a typed command line
    /// are two different intended creates, and the supervisor folds the
    /// mode into its own idempotency fingerprint precisely so a retry cannot
    /// flip between them.
    #[farhelm_testtrace::test]
    fn an_intent_binding_changes_with_the_host_incarnation_and_with_the_fields() {
        let hosts = [option(1, "this machine", true)];
        let command = || LaunchIntent::Command(test_command("agent"));
        let mut draft = LaunchDraft::new(None);
        draft.edit_folder("/tmp".into());
        draft.edit_title("title".into());
        let base = draft
            .request(RequestContext {
                host: Some((hosts[0].id, &hosts[0].incarnation)),
                launch: command(),
                replace_source: None,
                checkout: None,
            })
            .expect("the selected host is in the list");

        // Same id, different incarnation: a retarget or an adopt.
        let moved = [HostOption {
            incarnation: "incarnation-after-the-retarget".to_string(),
            ..hosts[0].clone()
        }];
        let after = draft
            .request(RequestContext {
                host: Some((moved[0].id, &moved[0].incarnation)),
                launch: command(),
                replace_source: None,
                checkout: None,
            })
            .expect("still selectable");
        assert_ne!(
            base, after,
            "the row is the same row; the machine behind it is not"
        );

        // Every form field is part of the intent too — this is what the
        // post-mint re-read compares against.
        for edited in [
            IntentBinding {
                conversation: Some("target".into()),
                ..base.clone()
            },
            IntentBinding {
                cwd: "/other".to_string(),
                ..base.clone()
            },
            IntentBinding {
                agent: LaunchIntent::Command(test_command("other-agent")),
                ..base.clone()
            },
            IntentBinding {
                agent: LaunchIntent::Structured(LaunchSelection {
                    permission_flags: None,
                    harness: LaunchHarness::Codex,
                    model: None,
                    effort: None,
                    permissions: None,
                    wrapper_prefix: None,
                    container: None,
                    workspace_trust: None,
                }),
                ..base.clone()
            },
            IntentBinding {
                title: "other title".to_string(),
                ..base.clone()
            },
            // A "replace with" is a different intent from the identical
            // plain create: reusing a plain create's key for a
            // replace-with (or the reverse) must mint a fresh one rather
            // than replay across the two verbs — see this field's own doc.
            IntentBinding {
                replace_source: Some("source-1".to_string()),
                ..base.clone()
            },
        ] {
            assert_ne!(base, edited);
        }

        // And an unchanged submit is the SAME intent, which is the whole
        // point of the key surviving a failure.
        assert_eq!(
            base,
            draft
                .request(RequestContext {
                    host: Some((hosts[0].id, &hosts[0].incarnation)),
                    launch: command(),
                    replace_source: None,
                    checkout: None,
                })
                .expect("still selectable")
        );
    }

    /// A successful Resume may finish before the form's host reader, even
    /// when a pending fixture already carries the returned token. Suppress
    /// only that same pending snapshot; a later token or settled state ends
    /// the handoff, without making the stale target available to submit.
    #[farhelm_testtrace::test]
    fn resumed_host_display_handoff_is_bound_to_one_pending_snapshot() {
        let mut connected = option(1, "sandbox", false);
        connected.connection = 9;
        let target = CreateTarget::new(1, connected.incarnation.clone());
        for token in [7, 9] {
            let mut pending = connected.clone();
            pending.connection = token;
            pending.phase = Some("Resuming…".into());
            pending.connection_pending = true;
            assert!(!pending.available_for_launch(None));
            let observation = resume_display_observation(std::slice::from_ref(&pending), Some(1));
            let receipt = completed_resume_display_handoff(
                Some(&target),
                observation.as_ref(),
                std::slice::from_ref(&connected),
            )
            .expect("the same installation completed resume");
            assert_eq!(Some(&receipt), observation.as_ref());
            pending.phase = Some("Connecting".into());
            assert_eq!(
                Some(receipt.clone()),
                resume_display_observation(std::slice::from_ref(&pending), Some(1))
            );
            assert!(!pending.available_for_launch(None));
            pending.connection = 10;
            assert_ne!(
                Some(receipt.clone()),
                resume_display_observation(std::slice::from_ref(&pending), Some(1))
            );
            for phase in [
                None,
                Some("Unreachable, retrying"),
                Some("Suspended"),
                Some("Retired"),
            ] {
                pending.phase = phase.map(str::to_string);
                pending.connection_pending = false;
                assert_eq!(
                    resume_display_observation(std::slice::from_ref(&pending), Some(1)),
                    None
                );
            }
        }
    }

    /// A target change or missing row retires display evidence rather than
    /// carrying one installation's successful Resume to its successor. An
    /// unsuccessful helper result must never manufacture a handoff receipt.
    #[farhelm_testtrace::test]
    fn resume_display_handoff_requires_success_on_the_unchanged_target() {
        let connected = option(1, "sandbox", false);
        let target = CreateTarget::new(1, connected.incarnation.clone());
        let mut pending = connected.clone();
        pending.phase = Some("Connecting".into());
        pending.connection_pending = true;
        let observation =
            resume_display_observation(std::slice::from_ref(&pending), Some(1)).unwrap();
        assert!(
            completed_resume_display_handoff(
                Some(&target),
                Some(&observation),
                std::slice::from_ref(&pending)
            )
            .is_none()
        );
        assert!(
            completed_resume_display_handoff(Some(&target), None, std::slice::from_ref(&connected))
                .is_none()
        );
        assert_eq!(resume_display_observation(&[], Some(1)), None);
        assert_eq!(
            resume_display_observation(std::slice::from_ref(&pending), Some(2)),
            None
        );
        pending.incarnation = "successor-installation".into();
        let successor = resume_display_observation(std::slice::from_ref(&pending), Some(1));
        assert_ne!(Some(&observation), successor.as_ref());
        assert!(
            completed_resume_display_handoff(
                Some(&target),
                successor.as_ref(),
                std::slice::from_ref(&connected)
            )
            .is_none()
        );
        let replaced = HostOption {
            incarnation: pending.incarnation.clone(),
            ..connected
        };
        assert!(
            completed_resume_display_handoff(Some(&target), Some(&observation), &[replaced])
                .is_none()
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

    /// A refusal about the folder marks the folder field, so the launcher
    /// must recognize the supervisor's three folder refusals however the helm
    /// wraps them, and nothing else. Specified: each fixed phrase is
    /// recognized inside a longer helm sentence and maps to its own problem;
    /// an unrelated refusal, and the words without their trailing ": ", are
    /// not folder refusals.
    #[farhelm_testtrace::test]
    fn folder_refusals_are_recognized_by_the_supervisor_words() {
        for (message, problem) in [
            (
                "working directory does not exist: /nope",
                FolderProblem::Missing,
            ),
            (
                "create refused: working directory is not a directory: /etc/hosts",
                FolderProblem::NotADirectory,
            ),
            (
                "working directory is not usable: /srv (permission denied); fix it and retry",
                FolderProblem::Unusable,
            ),
        ] {
            assert_eq!(
                FolderProblem::recognize(message),
                Some(problem),
                "{message}"
            );
        }
        for message in [
            "host is not connected",
            "the working directory does not exist yet",
            "working directory does not exist",
        ] {
            assert_eq!(FolderProblem::recognize(message), None, "{message}");
        }
    }

    /// A never-connected host yields no connection claim; a connected one
    /// yields exactly its token.
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

    /// Recent setups for one host, newest first; `true` marks a fresh managed
    /// checkout, whose folder Farhelm allocated.
    fn folder_history(rows: &[(&str, bool)]) -> api::LaunchHistory {
        let launches = rows
            .iter()
            .map(|(cwd, fresh)| api::LaunchHistoryEntry {
                target: None,
                host: 7,
                github_repo: fresh
                    .then(|| crate::github_checkout::GithubRepo::parse("acme/repo").unwrap()),
                canonical_cwd: None,
                cwd: (*cwd).into(),
                selection: farhelm_proto::launch::LaunchSelection {
                    harness: LaunchHarness::Claude,
                    model: None,
                    effort: None,
                    permissions: None,
                    permission_flags: None,
                    wrapper_prefix: None,
                    container: None,
                    workspace_trust: None,
                },
                created_at: 1,
                creation_seq: None,
            })
            .collect();
        api::LaunchHistory {
            launches,
            ..Default::default()
        }
    }

    /// An empty environment response must leave peer sequence order intact.
    /// A newer sequence may carry an earlier wall-clock time after a clock jump;
    /// sorting the combined input would pick the wrong recent representative.
    #[farhelm_testtrace::test]
    fn recent_merge_preserves_peer_order_when_environment_history_is_empty() {
        let mut ordinary =
            folder_history(&[("/newer-sequence", false), ("/older-sequence", false)]).launches;
        ordinary[0].created_at = 10;
        ordinary[0].creation_seq = Some(2);
        ordinary[1].created_at = 20;
        ordinary[1].creation_seq = Some(1);
        assert!(ordinary[0].creation_seq > ordinary[1].creation_seq);
        assert!(ordinary[0].created_at < ordinary[1].created_at);
        assert_eq!(merge_recent_launches(ordinary.clone(), &[]), ordinary);
    }

    /// Cross-source recency chooses between current heads only. Neither input
    /// may be internally reordered, and a newer environment must not be hidden
    /// behind every ordinary row merely because it came from another endpoint.
    #[farhelm_testtrace::test]
    fn recent_merge_interleaves_heads_and_preserves_both_source_orders() {
        let mut ordinary = folder_history(&[
            ("/ordinary-a", false),
            ("/ordinary-b", false),
            ("/ordinary-c", false),
        ])
        .launches;
        for (index, at) in [10, 50, 5].into_iter().enumerate() {
            ordinary[index].created_at = at;
            ordinary[index].creation_seq = Some(9 - index as u64);
        }
        let mut environment = folder_history(&[
            ("/environment-a", false),
            ("/environment-b", false),
            ("/environment-c", false),
        ])
        .launches;
        for (index, at) in [30, 7, 40].into_iter().enumerate() {
            environment[index].target = Some(farhelm_proto::LaunchTarget::Environment {
                recipe: "dev".into(),
                place: farhelm_proto::Place::PodmanHost(7),
            });
            environment[index].created_at = at;
            environment[index].creation_seq = Some(100 - index as u64);
        }
        let merged = merge_recent_launches(ordinary.clone(), &environment);
        assert_eq!(
            merged
                .iter()
                .map(|entry| entry.cwd.raw())
                .collect::<Vec<_>>(),
            [
                "/environment-a",
                "/ordinary-a",
                "/ordinary-b",
                "/environment-b",
                "/environment-c",
                "/ordinary-c"
            ]
        );
        assert_eq!(
            merged
                .iter()
                .filter(|entry| entry.target.is_none())
                .cloned()
                .collect::<Vec<_>>(),
            ordinary
        );
        assert_eq!(
            merged
                .iter()
                .filter(|entry| entry.target.is_some())
                .cloned()
                .collect::<Vec<_>>(),
            environment
        );
    }

    /// Helm-wide environment intent survives the absence of an ordinary host
    /// without becoming evidence for that host's remembered default folder.
    #[farhelm_testtrace::test]
    fn recent_projection_keeps_environment_and_default_folder_authority_separate() {
        let ordinary = folder_history(&[("/ordinary-remembered", false)]);
        let target = CreateTarget::new(7, "ordinary-install".into());
        let offered = (target.clone(), ordinary.clone());
        let mut environment = folder_history(&[("~", false)]).launches;
        environment[0].target = Some(farhelm_proto::LaunchTarget::Environment {
            recipe: "dev".into(),
            place: farhelm_proto::Place::PodmanHost(8),
        });
        environment[0].created_at = 2;
        let suggestions =
            offered_recent_history(Some(&offered), Some(&target), Some(&target), &environment);
        assert_eq!(suggestions.launches[0], environment[0]);
        assert_eq!(remembered_folder(&offered.1), "/ordinary-remembered");
        assert_eq!(
            offered.1, ordinary,
            "a suggestion projection cannot rewrite default-folder authority"
        );
        let independent = offered_recent_history(Some(&offered), None, None, &environment);
        assert_eq!(independent.launches, environment);
        assert!(independent.folders.is_empty());
    }

    /// Empty success is a frozen offer, while pending/failure retains the last
    /// successful fetched data. The real ChoiceTargets promoter alone reveals
    /// later entries or an empty replacement, even without an ordinary target.
    #[farhelm_testtrace::test]
    fn environment_history_retains_success_and_promotes_empty_deliberately() {
        use super::*;
        #[derive(Clone, Copy)]
        struct Fixture {
            choices: ChoiceTargets,
            revision: Signal<u64>,
        }
        std::thread_local! {
            static FIXTURE: std::cell::Cell<Option<Fixture>> = const { std::cell::Cell::new(None) };
        }
        /// Mount actual history handles without an ordinary installation or
        /// reader; environment promotion must not depend on either existing.
        fn fixture() -> Element {
            let choices = ChoiceTargets {
                partial_template_hint: use_signal(|| false),
                offered_history: use_signal(|| None),
                create_target: use_signal(|| None),
                fetched_history: use_signal(|| None),
                offered_environment_history: use_signal(|| None),
                fetched_environment_history: use_signal(|| None),
            };
            let revision = use_signal(|| 0);
            FIXTURE.with(|slot| slot.set(Some(Fixture { choices, revision })));
            rsx! {}
        }
        let mut dom = VirtualDom::new(fixture);
        dom.rebuild_in_place();
        let f = FIXTURE.with(std::cell::Cell::get).unwrap();
        dom.in_runtime(|| {
            let observe = |answer: Option<&Result<Vec<api::LaunchHistoryEntry>, String>>| {
                observe_environment_history(
                    answer,
                    f.choices.fetched_environment_history,
                    f.choices.offered_environment_history,
                    f.revision,
                )
            };
            observe(None);
            assert!(f.choices.offered_environment_history.peek().is_none());
            observe(Some(&Err("initial read refused".into())));
            assert!(f.choices.offered_environment_history.peek().is_none());
            assert_eq!(*f.revision.peek(), 0);
            observe(Some(&Ok(Vec::new())));
            assert_eq!(
                *f.choices.offered_environment_history.peek(),
                Some(Vec::new())
            );
            let mut entries = folder_history(&[("~", false)]).launches;
            entries[0].target = Some(farhelm_proto::LaunchTarget::Environment {
                recipe: "dev".into(),
                place: farhelm_proto::Place::PodmanHost(8),
            });
            observe(Some(&Ok(entries.clone())));
            assert_eq!(
                *f.choices.fetched_environment_history.peek(),
                Some(entries.clone())
            );
            assert_eq!(
                *f.choices.offered_environment_history.peek(),
                Some(Vec::new())
            );
            observe(Some(&Err("read refused".into())));
            observe(None);
            assert_eq!(
                *f.choices.fetched_environment_history.peek(),
                Some(entries.clone())
            );
            assert_eq!(
                *f.revision.peek(),
                2,
                "failure and pending reads do not invent a success"
            );
            f.choices.promote_environment_history();
            assert_eq!(
                *f.choices.offered_environment_history.peek(),
                Some(entries.clone())
            );
            observe(Some(&Ok(Vec::new())));
            assert_eq!(*f.choices.offered_environment_history.peek(), Some(entries));
            f.choices.promote_environment_history();
            assert_eq!(
                *f.choices.offered_environment_history.peek(),
                Some(Vec::new())
            );
        });
        FIXTURE.with(|slot| slot.set(None));
    }

    /// The per-host last folder is the newest plain-folder launch, so a fresh
    /// managed checkout, whose folder Farhelm allocated, never becomes the
    /// default; with no such launch the host's own home (`~`) is used.
    #[farhelm_testtrace::test]
    fn remembered_folder_is_the_newest_plain_folder_launch() {
        let history = folder_history(&[
            ("/work/fresh", true),
            ("/work/b", false),
            ("/work/a", false),
        ]);
        assert_eq!(remembered_folder(&history), "/work/b");
        assert_eq!(
            remembered_folder(&folder_history(&[("/work/fresh", true)])),
            "~"
        );
        assert_eq!(remembered_folder(&api::LaunchHistory::default()), "~");
    }

    /// One render of `folder_default_step`, spelled positionally for the table
    /// of cases below.
    fn step(
        state: FolderDefault,
        target: &CreateTarget,
        prefill: Option<u64>,
        host_is_users: bool,
        selected: Option<&(CreateTarget, String)>,
        offered: Option<&(CreateTarget, api::LaunchHistory)>,
        unreadable: Option<&CreateTarget>,
    ) -> (FolderDefault, Option<String>) {
        folder_default_step(
            state,
            FolderDefaultInputs {
                target: Some(target),
                prefill,
                host_is_users,
                selected,
                offered,
                unreadable,
                restored: None,
            },
        )
    }

    /// The default folder follows SPEC.md's precedence and stays put once a
    /// draft has it: an opening without a source folder seeds once from the
    /// first reply for its target, a later reply for the same target (a feed
    /// refresh) seeds nothing, and a host change re-arms the seed. An opening
    /// that brings a folder, or a Clone/Replace source, keeps it until the
    /// person changes the host; a Clone's own binding to its source host is not
    /// such a change. Returning to the selected session's installation restores
    /// its folder ahead of history, and an unreadable history falls back to
    /// home and says so, so a hidden quick launch never launches into a guess.
    #[farhelm_testtrace::test]
    fn folder_default_follows_precedence_once_per_opening_or_host_change() {
        let hosts = [option(7, "laptop", false), option(8, "linux", false)];
        let (laptop, linux) = (
            history_target(&hosts, Some(7)).unwrap(),
            history_target(&hosts, Some(8)).unwrap(),
        );
        let laptop_reply = (laptop.clone(), folder_history(&[("/Users/me/src", false)]));
        let linux_reply = (linux.clone(), folder_history(&[]));
        let fresh = FolderDefault {
            pending: true,
            ..FolderDefault::default()
        };

        let (state, seed) = step(fresh.clone(), &laptop, None, true, None, None, None);
        assert_eq!(seed, None, "nothing to seed before the reply arrives");
        let (state, seed) = step(state, &laptop, None, true, None, Some(&laptop_reply), None);
        assert_eq!(seed.as_deref(), Some("/Users/me/src"));
        let (state, seed) = step(state, &laptop, None, true, None, Some(&laptop_reply), None);
        assert_eq!(
            seed, None,
            "a later reply for the same target never reseeds"
        );
        let (state, seed) = step(state, &linux, None, true, None, Some(&laptop_reply), None);
        assert_eq!(seed, None, "another target's reply cannot seed");
        let (_, seed) = step(state, &linux, None, true, None, Some(&linux_reply), None);
        assert_eq!(
            seed.as_deref(),
            Some("~"),
            "a host change re-seeds, here to home"
        );

        // The selected session's folder wins on its own installation, also
        // when the person comes back to it, and without waiting for history.
        let selected = (laptop.clone(), "/work/current".to_string());
        let (state, _) = step(
            FolderDefault::default(),
            &laptop,
            None,
            true,
            Some(&selected),
            None,
            None,
        );
        let (state, seed) = step(
            state,
            &linux,
            None,
            true,
            Some(&selected),
            Some(&linux_reply),
            None,
        );
        assert_eq!(seed.as_deref(), Some("~"));
        let (_, seed) = step(state, &laptop, None, true, Some(&selected), None, None);
        assert_eq!(seed.as_deref(), Some("/work/current"));

        // A Clone/Replace source owns the folder until the person moves it.
        let (state, _) = step(fresh.clone(), &laptop, None, true, None, None, None);
        let (state, seed) = step(
            state,
            &laptop,
            Some(3),
            false,
            None,
            Some(&laptop_reply),
            None,
        );
        assert_eq!(
            seed, None,
            "a Clone/Replace source supersedes the pending seed"
        );
        let (bound, seed) = step(
            state,
            &linux,
            Some(3),
            false,
            None,
            Some(&linux_reply),
            None,
        );
        assert_eq!(
            seed, None,
            "a Clone binding to its source host keeps the source folder"
        );
        let (_, seed) = step(
            bound,
            &laptop,
            Some(3),
            true,
            None,
            Some(&laptop_reply),
            None,
        );
        assert_eq!(
            seed.as_deref(),
            Some("/Users/me/src"),
            "a host the person chose re-seeds a Clone draft too"
        );

        // A history that cannot be read falls back to home, marked unread.
        let (state, seed) = step(
            fresh.clone(),
            &laptop,
            None,
            true,
            None,
            None,
            Some(&laptop),
        );
        assert_eq!(seed.as_deref(), Some("~"));
        assert!(state.unread && !state.pending);

        // A permission Undo restores the folder default captured with its
        // snapshot; `undo` is one render restoring `snapshot` over `live`.
        let undo = |live: &FolderDefault,
                    snapshot: &FolderDefault,
                    offered: Option<&(CreateTarget, api::LaunchHistory)>| {
            folder_default_step(
                live.clone(),
                FolderDefaultInputs {
                    target: Some(&laptop),
                    prefill: None,
                    host_is_users: true,
                    selected: None,
                    offered,
                    unreadable: None,
                    restored: Some(snapshot),
                },
            )
        };
        let (settled, _) = step(
            fresh.clone(),
            &laptop,
            None,
            true,
            None,
            Some(&laptop_reply),
            None,
        );
        let later_reply = (
            laptop.clone(),
            folder_history(&[("/Users/me/later", false)]),
        );
        let (_, seed) = undo(&settled, &settled, Some(&later_reply));
        assert_eq!(
            seed.as_deref(),
            Some("/Users/me/src"),
            "a folder decided at the switch is applied again, never a later refresh"
        );
        let (pending_at_switch, _) = step(fresh, &laptop, None, true, None, None, None);
        let (after, seed) = undo(&settled, &pending_at_switch, Some(&later_reply));
        assert_eq!(
            seed.as_deref(),
            Some("/Users/me/src"),
            "a default pending at the switch takes the decision made since"
        );
        assert!(!after.pending);
        // A Clone's settled source folder at the switch stays, even after a
        // later host round trip decided another folder for the same target.
        let source_owned = FolderDefault {
            target: Some(laptop.clone()),
            ..FolderDefault::default()
        };
        let (after, seed) = undo(&settled, &source_owned, Some(&laptop_reply));
        assert_eq!(seed, None);
        assert_eq!(after.decided, None);
        assert!(!after.pending);
        // A snapshot naming another target than the restored destination
        // (captured before that destination was decided) keeps the decision
        // already made for the destination, not a later refresh.
        let stale = FolderDefault {
            target: Some(linux.clone()),
            decided: Some("~".to_string()),
            ..FolderDefault::default()
        };
        let (after, seed) = undo(&settled, &stale, Some(&later_reply));
        assert_eq!(seed.as_deref(), Some("/Users/me/src"));
        assert_eq!(after.target.as_ref(), Some(&laptop));
        // A Clone source that still owns placement after its host was withdrawn
        // automatically keeps its settled folder across a mismatched restore.
        let source_on_linux = FolderDefault {
            target: Some(linux.clone()),
            prefill: Some(3),
            ..FolderDefault::default()
        };
        let withdrawn = FolderDefault {
            target: Some(laptop.clone()),
            prefill: Some(3),
            ..FolderDefault::default()
        };
        let selected = (laptop.clone(), "/work/selected".to_string());
        let (after, seed) = folder_default_step(
            withdrawn,
            FolderDefaultInputs {
                target: Some(&laptop),
                prefill: Some(3),
                host_is_users: false,
                selected: Some(&selected),
                offered: Some(&laptop_reply),
                unreadable: None,
                restored: Some(&source_on_linux),
            },
        );
        assert_eq!(seed, None);
        assert!(!after.pending);
        // A source-owned move to another target never carries a decision along.
        let (moved, _) = step(settled, &linux, None, false, None, None, None);
        assert_eq!(moved.decided, None);
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

    /// Hostless bodies would silently default at the helm. The request
    /// projection must preserve absence for both no selection and a vanished row.
    #[farhelm_testtrace::test]
    fn no_selected_host_yields_no_binding() {
        let hosts = [option(1, "this machine", true)];
        let draft = LaunchDraft::new(None);
        for selected in [None, Some(99)] {
            let host = hosts.iter().find(|host| Some(host.id) == selected);
            assert!(
                draft
                    .request(RequestContext {
                        host: host.map(|host| (host.id, host.incarnation.as_str())),
                        launch: LaunchIntent::Command(test_command("")),
                        replace_source: None,
                        checkout: None,
                    })
                    .is_none(),
                "an absent selected row cannot supply a request target"
            );
        }
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

    /// The raw invocation is carried on every prefill, a structured one
    /// included, which has no use for it until the user switches the mounted
    /// form to the command tab: leaving it unset there would let a stale,
    /// unrelated command surface then.
    #[farhelm_testtrace::test]
    fn prefill_from_carries_the_raw_invocation_even_for_a_structured_clone() {
        let session = Session {
            invocation: "claude --resume abc".into(),
            launch: Some(crate::SessionLaunch::Agent {
                start: Vec::new(),
                resume: None,
                selection: LaunchSelection {
                    permission_flags: None,
                    harness: LaunchHarness::Claude,
                    model: None,
                    effort: None,
                    permissions: None,
                    wrapper_prefix: None,
                    container: None,
                    workspace_trust: None,
                },
            }),
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
            permission_flags: None,
            harness: LaunchHarness::Muse,
            model: Some("muse-spark-1.3-contributor".to_string()),
            effort: None,
            permissions: Some(LaunchPermission::Yolo),
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        };
        let session = Session {
            invocation: "muse --model muse-spark-1.3-contributor --yolo".into(),
            launch: Some(crate::SessionLaunch::Agent {
                selection: launch.clone(),
                start: Vec::new(),
                resume: None,
            }),
            ..row_specimen("structured")
        };

        assert_eq!(prefill_from(&session, 1).launch, Some(launch));
        assert_eq!(prefill_from(&session, 1).command, None);
    }

    /// A command launch's clone carries its whole command launch (command,
    /// YOLO assertion, declared agent type, resume command), and a legacy
    /// session's carries its command text alone.
    ///
    /// Why: SPEC.md has Clone carry a command launch's fields verbatim, and
    /// open a legacy session's launcher with the stored command filled in
    /// and nothing else; a legacy assertion would be a guess.
    #[farhelm_testtrace::test]
    fn prefill_from_carries_a_command_launch_whole_and_a_legacy_command_alone() {
        let command = crate::CommandLaunch {
            command: "claude --x {farhelm_args}".to_string(),
            yolo: true,
            agent: Some(LaunchHarness::Claude),
            resume: Some("claude --resume {conversation} {farhelm_args}".to_string()),
        };
        let session = Session {
            invocation: command.command.clone().into(),
            launch: Some(crate::SessionLaunch::Command(command.clone())),
            ..row_specimen("command")
        };
        let prefill = prefill_from(&session, 1);
        assert_eq!(prefill.command, Some(command));
        assert_eq!(prefill.launch, None);

        let legacy = Session {
            invocation: "claude --model opus".into(),
            launch: Some(crate::SessionLaunch::Legacy {
                invocation: "claude --model opus".to_string(),
                agent_kind: farhelm_proto::AgentKind::Claude,
                resume_template: None,
            }),
            ..row_specimen("legacy")
        };
        let prefill = prefill_from(&legacy, 1);
        assert_eq!(prefill.invocation, "claude --model opus");
        assert_eq!(prefill.command, None);
        assert_eq!(prefill.launch, None);
    }

    /// Everything else on a prefill travels off the row unmodified — no
    /// suffix on the title, no rewriting of the directory, the row's own
    /// host and install identity together — and the generation is exactly
    /// what the caller passed in (`ListView` is the one that decides what
    /// counts as a new clone). The mode starts as Create: only the
    /// Replace-with or Restart-with handler changes the operation afterward,
    /// so a clone cannot accidentally relaunch or delete its source.
    #[farhelm_testtrace::test]
    fn prefill_from_carries_title_cwd_host_and_identity_verbatim() {
        let session = Session {
            cwd: "/work/api".into(),
            title: "my session".into(),
            host: Some(7),
            host_identity: Some(Some("install-7".into())),
            ..row_specimen("s1")
        };
        let prefill = prefill_from(&session, 3);
        assert_eq!(prefill.generation, 3);
        assert_eq!(prefill.host, Some(7));
        assert_eq!(prefill.host_identity, Some(Some("install-7".to_string())));
        assert_eq!(prefill.cwd, "/work/api");
        assert_eq!(prefill.title, "my session");
        assert_eq!(prefill.mode, LauncherMode::Create);
    }

    /// Why this matters: Clone and Replace with copy the source's title, and a fresh checkout names
    /// its directory after the title and refuses a taken one, so the copied title (usually the
    /// source's own checkout) made every Clone or Replace with into `gh:` of the same repository
    /// fail with a directory conflict. Spec (SPEC.md, Managed checkouts): with a fresh checkout as
    /// the destination, an unedited copied title is sent empty so the session is unnamed and gets
    /// `repo-N`; an edited title, a form with no copied seed, and every existing-folder launch keep
    /// `submitted_field`'s rule unchanged.
    #[farhelm_testtrace::test]
    fn a_copied_unedited_title_is_sent_empty_only_for_a_fresh_checkout() {
        let copied = Some("bar-fix");
        // Fresh checkout, copied and untouched: unnamed.
        assert!(copied_title_ignored(false, copied, true));
        assert_eq!(submitted_title("bar-fix", false, copied, true, None), "");
        // The same field with a folder destination keeps the copied raw
        // title, including when its display spelling is escaped.
        assert!(!copied_title_ignored(false, copied, false));
        assert_eq!(
            submitted_title("escaped", false, copied, false, None),
            "bar-fix"
        );
        // Typing makes the title the person's own again, for either
        // destination, so a taken typed name still gets the conflict.
        assert!(!copied_title_ignored(true, copied, true));
        assert_eq!(submitted_title("typed", true, copied, true, None), "typed");
        assert_eq!(submitted_title("typed", true, copied, false, None), "typed");
        // An ordinary New form has no seed: whatever the field holds is sent.
        assert!(!copied_title_ignored(false, None, true));
        assert_eq!(submitted_title("", false, None, true, None), "");
        assert_eq!(submitted_title("named", false, None, true, None), "named");
    }

    /// Template snapshots must offer the same effective name as a launch,
    /// including an untouched Clone suffix or an intentionally unnamed checkout.
    /// Returning to a directory still keeps the source's original raw bytes.
    #[test]
    fn effective_title_keeps_clone_defaults_and_ignored_checkout_names() {
        let repo = GithubRepo::parse("acme/source").unwrap();
        let source = DestinationDraft::github(repo.clone());
        let other = DestinationDraft::github(GithubRepo::parse("acme/other").unwrap());
        let folder = DestinationDraft::Existing {
            cwd: "/work".into(),
        };
        let mut naming = CheckoutNaming {
            source: Some(CheckoutNameSource::Clone(repo)),
            ..Default::default()
        };
        let raw = "source\tname";
        assert_eq!(
            effective_title("escaped", false, Some(raw), &source, &naming),
            "source\tname-clone"
        );
        assert!(naming.advance(false));
        assert_eq!(
            effective_title("escaped", false, Some(raw), &source, &naming),
            "source\tname-clone-2"
        );
        assert_eq!(
            effective_title("escaped", false, Some(raw), &other, &naming),
            ""
        );
        assert_eq!(
            effective_title("escaped", false, Some(raw), &folder, &naming),
            raw
        );
        assert_eq!(
            effective_title("typed", true, Some(raw), &source, &naming),
            "typed"
        );
    }

    /// Template bases share Clone's bounded search, while fixed names keep
    /// their exact title. Folder destinations do not number names, and the
    /// source base survives a resolved suffix for Save as template.
    #[test]
    fn template_base_names_number_only_for_managed_destinations() {
        use farhelm_proto::launcher::TemplateNameKind;
        let repo = GithubRepo::parse("acme/web").unwrap();
        let checkout = DestinationDraft::github(repo.clone());
        let folder = DestinationDraft::Existing {
            cwd: "/work".into(),
        };
        let mut naming = CheckoutNaming {
            source: Some(CheckoutNameSource::Template {
                base: "app-review".into(),
                kind: TemplateNameKind::Base,
            }),
            ..Default::default()
        };
        naming.retarget(
            Some(CreateTarget::new(1, "install-a".into())),
            Some(repo.clone()),
        );
        assert_eq!(
            effective_title("app-review", true, None, &checkout, &naming),
            "app-review"
        );
        assert!(
            !naming.advance(true),
            "an unresolved create must keep its displayed name"
        );
        assert!(naming.advance(false));
        assert_eq!(
            effective_title("app-review", true, None, &checkout, &naming),
            "app-review-2"
        );
        assert_eq!(
            naming.template_name(),
            Some(("app-review", TemplateNameKind::Base))
        );
        assert_eq!(
            effective_title("app-review", true, None, &folder, &naming),
            "app-review"
        );
        naming.retarget(Some(CreateTarget::new(2, "install-b".into())), Some(repo));
        assert_eq!(
            effective_title("app-review", true, None, &checkout, &naming),
            "app-review"
        );
        naming.source = Some(CheckoutNameSource::Template {
            base: "exact".into(),
            kind: TemplateNameKind::Fixed,
        });
        assert_eq!(
            effective_title("exact", true, None, &checkout, &naming),
            "exact"
        );
        assert!(naming.default_title(true, None, checkout.repo()).is_none());
        assert_eq!(
            naming.template_name(),
            Some(("exact", TemplateNameKind::Fixed))
        );
    }

    /// Clone's default may search for a free checkout name. Switching
    /// hosts or repositories restarts that search; explicit edits, unnamed
    /// sources and Replace with must never acquire an automatic suffix.
    #[test]
    fn clone_checkout_names_are_defaults_scoped_to_the_destination() {
        let repo = GithubRepo::parse("acme/bar").unwrap();
        let other = GithubRepo::parse("acme/other").unwrap();
        let host = Some(CreateTarget::new(1, "install-a".into()));
        let mut naming = CheckoutNaming {
            source: Some(CheckoutNameSource::Clone(repo.clone())),
            ..Default::default()
        };
        naming.retarget(host.clone(), Some(repo.clone()));
        let default = naming
            .default_title(false, Some("bar-3"), Some(&repo))
            .unwrap();
        assert_eq!(default, "bar-3-clone");
        assert_eq!(
            submitted_title("bar-3", false, Some("bar-3"), true, Some(&default)),
            default
        );
        assert!(
            !naming.advance(true),
            "a retained request must keep its original name"
        );
        assert!(!naming.sequence.is_numbered());
        assert!(naming.advance(false));
        assert_eq!(
            naming
                .default_title(false, Some("bar-3"), Some(&repo))
                .as_deref(),
            Some("bar-3-clone-2")
        );
        naming.retarget(host.clone(), Some(repo.clone()));
        assert_eq!(
            naming.sequence.name("base"),
            "base-2",
            "same destination keeps its search"
        );
        naming.retarget(
            Some(CreateTarget::new(2, "install-b".into())),
            Some(repo.clone()),
        );
        assert_eq!(
            naming.sequence.name("base"),
            "base",
            "new host searches from the base name"
        );
        assert!(
            naming
                .default_title(true, Some("bar-3"), Some(&repo))
                .is_none()
        );
        assert!(naming.default_title(false, Some(""), Some(&repo)).is_none());
        assert!(naming.default_title(false, Some("bar-3"), None).is_none());
        assert!(
            naming
                .default_title(false, Some("bar-3"), Some(&other))
                .is_none()
        );
        naming.advance(false);
        naming.retarget(host, Some(other));
        assert_eq!(
            naming.sequence.name("base"),
            "base",
            "repository changes reset the counter"
        );
        naming.source = None;
        assert!(
            naming
                .default_title(false, Some("bar-3"), Some(&repo))
                .is_none()
        );
        for _ in 0..49 {
            assert!(naming.advance(false));
        }
        assert_eq!(naming.sequence.name("base"), "base-50");
        assert!(
            !naming.advance(false),
            "cap leaves a conflict for the person to fix"
        );
    }

    /// Why this matters: "Replace with" opened the launcher straight from the
    /// menu and its replace killed whatever the source was running, with
    /// nothing on screen saying so (SPEC.md "Replace with"). Spec: a source
    /// with anything alive gets Replace's warning and the precondition that
    /// warning covers (a running agent: unconditional; an ended agent with a
    /// tab: agent-ended); a source showing nothing alive, or no state at
    /// all, gets no warning and the nothing-alive precondition, so a restart
    /// the launcher did not show refuses the delete.
    #[farhelm_testtrace::test]
    fn replace_with_warns_and_binds_to_the_source_state_it_shows() {
        let running = (SessionStatus::Running, 0);
        let (warning, guard) = replace_with_warning(Some(&running));
        assert!(
            warning
                .as_deref()
                .is_some_and(|text| text.starts_with("Still running")),
            "{warning:?}"
        );
        assert_eq!(guard, crate::DeleteGuard::Unconditional);

        let tabs_only = (SessionStatus::Exited { exit_code: Some(0) }, 1);
        let (warning, guard) = replace_with_warning(Some(&tabs_only));
        assert!(
            warning
                .as_deref()
                .is_some_and(|text| text.starts_with("1 terminal tab is still open")),
            "{warning:?}"
        );
        assert_eq!(guard, crate::DeleteGuard::AgentEnded);

        let nothing_alive = (SessionStatus::Exited { exit_code: Some(0) }, 0);
        assert_eq!(
            replace_with_warning(Some(&nothing_alive)),
            (None, crate::DeleteGuard::NothingAlive)
        );
        assert_eq!(
            replace_with_warning(None),
            (None, crate::DeleteGuard::NothingAlive)
        );
    }

    /// Why this matters: the launcher falls back to the state its source had
    /// when it opened whenever the live listing lacks the row, so that
    /// snapshot must be taken from the row the user picked. Spec: marking a
    /// clone's prefill as a replace-with records the row's id, status and
    /// tab count, and leaves the clone's other fields as `prefill_from` set
    /// them.
    #[farhelm_testtrace::test]
    fn marking_a_replace_with_records_the_source_and_its_state() {
        let session = Session {
            status: SessionStatus::Exited { exit_code: Some(1) },
            tabs: vec![crate::Tab { id: "t1".into() }],
            ..row_specimen("s1")
        };
        let clone = prefill_from(&session, 4);
        let mut prefill = clone.clone();
        mark_replace_with(&mut prefill, &session);
        assert_eq!(prefill.mode.replace_source(), Some("s1"));
        assert_eq!(
            prefill.mode.source_opened(),
            Some(&(SessionStatus::Exited { exit_code: Some(1) }, 1))
        );
        assert_eq!(
            CreatePrefill {
                mode: LauncherMode::Create,
                ..prefill
            },
            clone
        );
    }

    /// A refused restart initialization must stay distinguishable from an
    /// ordinary create prefill. The list uses this result to avoid opening a
    /// new-session editor in response to an existing-session action.
    #[farhelm_testtrace::test]
    fn marking_restart_refuses_sources_without_an_editable_resume() {
        for launch in [
            None,
            Some(crate::SessionLaunch::Command(crate::CommandLaunch {
                command: "claude {farhelm_args}".into(),
                agent: Some(LaunchHarness::Claude),
                resume: None,
                yolo: false,
            })),
        ] {
            let session = Session {
                launch,
                ..row_specimen("s1")
            };
            let mut prefill = prefill_from(&session, 4);
            assert!(!mark_restart_with(&mut prefill, &session));
            assert_eq!(prefill.mode, LauncherMode::Create);
        }
    }

    /// Checkout association chooses Clone's new repository, including a
    /// borrower below its root. Keep the source cwd too: Replace with and
    /// switching back to a folder need the original subdirectory unchanged.
    #[farhelm_testtrace::test]
    fn prefill_from_checkout_metadata_keeps_existing_subdirectory() {
        let mut session = Session {
            cwd: "/work/bar-1/subdir".into(),
            working_copy: Some(crate::github_checkout::WorkingCopyInfo {
                id: "checkout-1".into(),
                repo: GithubRepo::parse("acme/bar").unwrap(),
                canonical_path: "/work/bar-1".into(),
                origin_session_id: "origin".into(),
            }),
            ..row_specimen("borrower")
        };
        let prefill = prefill_from(&session, 1);
        assert_eq!(prefill.cwd, "/work/bar-1/subdir");
        assert_eq!(prefill.repo, Some(GithubRepo::parse("acme/bar").unwrap()));
        session.github_repo = Some(GithubRepo::parse("other/provenance").unwrap());
        assert_eq!(
            prefill_from(&session, 2).repo,
            Some(GithubRepo::parse("acme/bar").unwrap())
        );
        session.working_copy = None;
        assert_eq!(prefill_from(&session, 3).repo, session.github_repo);
    }

    /// Session metadata bypasses repository search validation on both Clone
    /// provenance paths. Only its editor spelling changes: the selected repo,
    /// matching query and serialized identity must keep the peer's exact bytes.
    /// Safe selections and unselected input keep their ordinary editing rules.
    #[test]
    fn cloned_repository_field_escapes_without_replacing_raw_identity() {
        let wire = serde_json::json!({ "owner": "acme", "name": "repo\u{202e}\u{200b}" });
        let repo: GithubRepo = serde_json::from_value(wire.clone()).unwrap();
        let raw = "acme/repo\u{202e}\u{200b}";
        assert!(
            GithubRepo::parse(raw).is_err(),
            "the fixture must bypass the search parser"
        );
        for working_copy in [false, true] {
            let session = Session {
                github_repo: (!working_copy).then(|| repo.clone()),
                working_copy: working_copy.then(|| crate::github_checkout::WorkingCopyInfo {
                    id: "checkout-1".into(),
                    repo: repo.clone(),
                    canonical_path: "/work/repo-1".into(),
                    origin_session_id: "origin".into(),
                }),
                ..row_specimen("source")
            };
            let inherited = prefill_from(&session, 1).repo.unwrap();
            let draft = DestinationDraft::github(inherited);
            assert_eq!(
                repository_field(&draft),
                ("acme/repo<U+202E><U+200B>".into(), true)
            );
            assert_eq!(draft.repository_text(), raw);
            assert_eq!(draft.repo().unwrap().identifier(), raw);
            assert_eq!(serde_json::to_value(draft.repo().unwrap()).unwrap(), wire);
        }
        let safe = DestinationDraft::github(GithubRepo::parse("acme/repo").unwrap());
        assert_eq!(repository_field(&safe), ("acme/repo".into(), false));
        let query = DestinationDraft::GithubInput { query: raw.into() };
        assert_eq!(repository_field(&query), (raw.into(), false));
        let cleared = DestinationDraft::GithubInput {
            query: String::new(),
        };
        assert_eq!(repository_field(&cleared), (String::new(), false));
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

    /// A clone must preserve original bytes from the session adapter through
    /// untouched submission, while an edit submits the user's draft verbatim.
    /// A directional override makes raw and displayed spellings differ, so
    /// accidentally copying escaped text into a raw seed cannot pass unnoticed.
    #[farhelm_testtrace::test]
    fn a_cloned_fields_untouched_submission_sends_the_original_bytes_not_the_escaped_display() {
        let raw = "claude --resume \u{202E}reversed-arg";
        let source = Session {
            cwd: raw.into(),
            title: raw.into(),
            invocation: raw.into(),
            ..row_specimen("clone-source")
        };
        let prefill = prefill_from(&source, 1);
        assert_eq!(prefill.cwd, raw);
        assert_eq!(prefill.title, raw);
        assert_eq!(prefill.invocation, raw);
        let display = display_peer(raw);
        assert_ne!(
            display, raw,
            "the escaped display must differ from the raw bytes for this test to mean anything"
        );

        // What the draft's raw seed transitions display, and what an
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

    /// Why this matters: a launcher search result's hover text can quote a
    /// value the user typed or a peer supplied (a session name), and a
    /// tooltip is one attribute string that no DOM direction isolation
    /// reaches, so escaping is its only defense against an override
    /// character reordering what the user reads. Spec: a name result's
    /// tooltip carries the escaped name, never the raw control character.
    #[farhelm_testtrace::test]
    fn search_result_tooltips_escape_names() {
        use crate::launch_composer::ComposerSearchResult;
        let spoof = "build\u{202E}lanif".to_string();
        let name =
            super::search_result_tooltip(&ComposerSearchResult::Name(spoof.clone()), "local", None);
        assert!(
            !name.contains('\u{202E}'),
            "a raw override reached the tooltip: {name:?}"
        );
        assert!(
            name.contains(&crate::peer::display_peer(&spoof)),
            "the escaped name is quoted: {name:?}"
        );
    }

    /// Spec (`LauncherYoloQuestion`): an answer to the launcher's YOLO
    /// question records consent, and asks for a submit, only by taking the
    /// opening it was drawn for. Cancel followed by either answer in one
    /// event burst records no consent, no "don't ask again" request, and no
    /// submit; an answer drawn for an earlier opening takes nothing from a
    /// later one; an answer whose render showed an earlier launcher opening
    /// (another source opened since, question still up) takes nothing; a
    /// genuine answer records consent for the refused request's key, and
    /// only "don't ask again" also asks to mark the host.
    ///
    /// Why: the recorded consent is what lets the form's next submit of
    /// that request launch with no approval prompts (and, from Replace
    /// with, delete the source session). The launcher used to record it
    /// from the answer's own copy of the question, so an answer queued
    /// behind Cancel restored the consent the user had just withdrawn and
    /// submitted the draft. Consent kept under the key after a genuine
    /// answer is what lets a retry of the same request keep it. An answer
    /// drawn for an earlier launcher opening would act on the earlier source,
    /// which the user has already moved away from, so it is refused too.
    #[farhelm_testtrace::test]
    fn a_cancelled_launcher_yolo_question_records_no_consent() {
        use dioxus::prelude::*;
        use std::cell::Cell;

        use super::{LauncherYoloQuestion, answer_launcher_yolo};
        use crate::ops::{ConfirmSlot, use_confirm_slot};

        std::thread_local! {
            static SLOT: Cell<Option<ConfirmSlot<LauncherYoloQuestion>>> =
                const { Cell::new(None) };
        }

        fn app() -> Element {
            let slot = use_confirm_slot::<LauncherYoloQuestion, ()>();
            SLOT.with(|cell| cell.set(Some(slot)));
            rsx! {}
        }

        let question = || LauncherYoloQuestion {
            refused_key: "intent-1".to_string(),
            ask: crate::yolo_confirm::YoloAsk {
                host: Some(7),
                host_name: "build box".to_string(),
                reason: crate::yolo_confirm::YoloReason::Asserted,
            },
            uncertain: false,
        };

        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        let mut slot = SLOT.with(Cell::get).expect("the slot mounted");

        dom.in_runtime(|| {
            for stop_asking in [false, true] {
                // An armed request the stale answer must leave alone, so a
                // helper that wrote it regardless would show. Its opening
                // is replaced by the one the test answers.
                let armed = slot.open(question(), ());
                let open = slot.open(question(), ());
                let (mut confirmed, mut stop_for) = (None, Some(armed.clone()));
                assert!(slot.take(&open).is_some(), "premise: Cancel closes it");
                assert!(
                    !answer_launcher_yolo(
                        &mut slot,
                        &open,
                        stop_asking,
                        1,
                        1,
                        &mut confirmed,
                        &mut stop_for
                    ),
                    "an answer queued behind Cancel (stop asking: {stop_asking}) does not submit"
                );
                assert_eq!(confirmed, None, "and records no consent");
                assert!(
                    stop_for == Some(armed),
                    "and changes no request to mark the host"
                );
            }

            // An answer drawn for an earlier opening of the same refused
            // request, replaced since.
            let earlier = slot.open(question(), ());
            let current = slot.open(question(), ());
            let (mut confirmed, mut stop_for) = (None, None);
            assert!(!answer_launcher_yolo(
                &mut slot,
                &earlier,
                true,
                1,
                1,
                &mut confirmed,
                &mut stop_for
            ));
            assert_eq!(confirmed, None);
            assert!(
                stop_for.is_none(),
                "an answer for an earlier opening marks no host"
            );
            assert!(slot.is_open(), "the later question is left alone");

            // The current opening, but the launcher has since moved to
            // another source (opening 2) without closing the question. The
            // answer's render showed opening 1's Replace warning, so it must
            // neither take the question nor record consent.
            for stop_asking in [false, true] {
                let (mut confirmed, mut stop_for) = (None, None);
                assert!(
                    !answer_launcher_yolo(
                        &mut slot,
                        &current,
                        stop_asking,
                        1,
                        2,
                        &mut confirmed,
                        &mut stop_for
                    ),
                    "an answer from a superseded launcher opening (stop asking: {stop_asking}) does not submit"
                );
                assert_eq!(confirmed, None);
                assert!(stop_for.is_none());
                assert!(
                    slot.peek_token() == Some(current.clone()),
                    "and leaves the question for the live opening's own answer"
                );
            }

            // Positive controls. The one-off records consent and disarms a
            // stray "don't ask again" request.
            let mut stop_for = Some(current.clone());
            assert!(answer_launcher_yolo(
                &mut slot,
                &current,
                false,
                1,
                1,
                &mut confirmed,
                &mut stop_for
            ));
            assert_eq!(confirmed.as_deref(), Some("intent-1"));
            assert!(stop_for.is_none(), "the one-off never marks the host");
            assert!(!slot.is_open());

            let permanent = slot.open(question(), ());
            let (mut confirmed, mut stop_for) = (None, None);
            assert!(answer_launcher_yolo(
                &mut slot,
                &permanent,
                true,
                1,
                1,
                &mut confirmed,
                &mut stop_for
            ));
            assert_eq!(confirmed.as_deref(), Some("intent-1"));
            assert!(stop_for.is_some_and(|answered| answered == permanent));
        });
    }

    /// Why: a Guarded answer in Replace with waits for a checkout preview
    /// before submitting, and the source can start again meanwhile. SPEC.md
    /// says the replace then carries the precondition the launcher showed at
    /// the click, so a source with more alive by then is kept.
    ///
    /// Spec: while the opening, preview authority (ignoring its generation),
    /// prompt and conversation are unchanged, the stamp yields the
    /// precondition it was created with as the one the deferred submit sends;
    /// any of those changing yields nothing to send.
    #[test]
    fn guarded_submit_stamp_sends_the_answered_precondition() {
        use super::*;
        let authority = |generation| PreviewAuthority {
            generation,
            config_revision: 1,
            host: "7".into(),
            incarnation: 2,
            installation_identity: "installation-a".into(),
            repo: GithubRepo::parse("acme/bar").unwrap(),
            title: None,
            agent: "codex".into(),
        };
        let prompt = Some("original prompt".to_string());
        let stamp = GuardedSubmitStamp::new(
            3,
            authority(5),
            prompt.clone(),
            "conversation".into(),
            crate::DeleteGuard::NothingAlive,
            Some(8),
        );
        // The live source may now imply `Unconditional`; the stamp's answer
        // to "what does the submit send" is still what the user saw.
        assert_eq!(
            stamp.guard_if_current(3, authority(6), &prompt, "conversation", None),
            Some(crate::DeleteGuard::NothingAlive)
        );
        for (opening, prompt, conversation) in [
            (4, prompt.clone(), "conversation"),
            (3, None, "conversation"),
            (3, prompt.clone(), "another"),
        ] {
            assert_eq!(
                stamp.guard_if_current(opening, authority(6), &prompt, conversation, None),
                None
            );
        }
        let mut moved = authority(6);
        moved.incarnation = 3;
        assert_eq!(
            stamp.guard_if_current(3, moved, &prompt, "conversation", None),
            None
        );
        let mut changed_config = authority(6);
        changed_config.config_revision += 1;
        assert!(
            stamp
                .guard_if_current(3, changed_config, &prompt, "conversation", None)
                .is_none(),
            "a guarded answer still binds the configuration it accepted"
        );
    }

    /// The 60-second limit after a wake may end only the wait for this window
    /// to see the woken host connect, never the checkout preview that follows
    /// it (SPEC.md "Creation"). A visible guarded answer, which never woke a
    /// host, is never in that wait. The wait holds while the form still sees
    /// the replaced connection or the host still shows a connection pending,
    /// and ends once the fresh connection is seen and settled.
    #[test]
    fn only_a_woken_press_short_of_its_connection_awaits_it() {
        let authority = PreviewAuthority {
            generation: 5,
            config_revision: 1,
            host: "7".into(),
            incarnation: 2,
            installation_identity: "installation-a".into(),
            repo: GithubRepo::parse("acme/bar").unwrap(),
            title: Some(String::new()),
            agent: "codex".into(),
        };
        let pending_display = ResumeDisplayHandoff {
            target: CreateTarget::new(7, "fingerprint".into()),
            connection: 4,
        };
        let answer = GuardedSubmitStamp::new(
            3,
            authority.clone(),
            None,
            String::new(),
            crate::DeleteGuard::NothingAlive,
            None,
        );
        assert!(
            !answer.awaiting_woken_connection(Some(&authority), Some(&pending_display)),
            "a press that woke nothing has no connection to wait for"
        );
        let woken = answer.after_resume(4, None);
        assert!(
            woken.awaiting_woken_connection(Some(&authority), None),
            "the form still sees the connection the wake replaced"
        );
        let mut fresh = authority.clone();
        fresh.incarnation = 4;
        assert!(
            woken.awaiting_woken_connection(Some(&fresh), Some(&pending_display)),
            "the fresh token is visible but the host still shows Connecting"
        );
        assert!(
            !woken.awaiting_woken_connection(Some(&fresh), None),
            "connected: the press now waits only for its checkout preview"
        );
        assert!(
            !woken.awaiting_woken_connection(None, None),
            "no authority and nothing pending on screen is not this wait"
        );
        // The slot-level rule the timer and the form's attribute both use.
        assert!(parked_press_awaiting_woken_connection(
            Some((&woken, true)),
            Some(&authority),
            None
        ));
        assert!(
            !parked_press_awaiting_woken_connection(Some((&woken, true)), Some(&fresh), None),
            "a parked press that saw its connection is never discarded by the limit"
        );
        assert!(
            !parked_press_awaiting_woken_connection(Some((&woken, false)), Some(&authority), None),
            "a press without its token is not the wake's parked press"
        );
        assert!(
            !parked_press_awaiting_woken_connection(None, Some(&authority), Some(&pending_display)),
            "an empty slot, such as a press that already continued, has nothing to discard"
        );
    }

    /// The error names the limit in words; it must say the same number of
    /// seconds the timer waits, or the launcher would misreport the wait.
    #[test]
    fn the_woken_connection_error_names_the_limit_it_enforces() {
        let seconds = WOKEN_CONNECTION_WAIT_MS / 1000;
        assert_eq!(WOKEN_CONNECTION_WAIT_MS % 1000, 0);
        assert!(WOKEN_CONNECTION_TIMEOUT.contains(&format!("within {seconds} seconds")));
    }

    /// Resume must survive a lagging host read without using its stale preview.
    /// Only the returned connection can submit; a changed opening, draft or
    /// installation cancels the press even during that display handoff.
    #[test]
    fn resumed_submit_stamp_waits_only_for_its_fresh_connection() {
        let authority = PreviewAuthority {
            generation: 5,
            config_revision: 1,
            host: "7".into(),
            incarnation: 2,
            installation_identity: "installation-a".into(),
            repo: GithubRepo::parse("acme/bar").unwrap(),
            title: Some(String::new()),
            agent: "codex".into(),
        };
        let stamp = GuardedSubmitStamp::new(
            3,
            authority.clone(),
            None,
            String::new(),
            crate::DeleteGuard::NothingAlive,
            Some(8),
        )
        .after_resume(4, None);
        assert_eq!(
            stamp.press_baseline,
            Some(8),
            "wake is preparation, not a new answer"
        );
        assert!(stamp.waiting_for_connection(&authority));
        assert!(
            stamp
                .guard_if_current(3, authority.clone(), &None, "", None)
                .is_some()
        );
        let mut fresh = authority.clone();
        fresh.incarnation = 4;
        fresh.generation = 6;
        fresh.config_revision = 7;
        assert!(!stamp.waiting_for_connection(&fresh));
        assert!(
            stamp
                .guard_if_current(3, fresh.clone(), &None, "", None)
                .is_some(),
            "learning the checkout configuration cannot cancel auto-allocation"
        );
        for mut changed in [authority.clone(), fresh.clone()] {
            changed.installation_identity = "another-installation".into();
            assert!(
                stamp
                    .guard_if_current(3, changed, &None, "", None)
                    .is_none()
            );
        }
        for connection in [1, 3, 5] {
            fresh.incarnation = connection;
            assert!(
                stamp
                    .guard_if_current(3, fresh.clone(), &None, "", None)
                    .is_none()
            );
        }
        for (opening, prompt, conversation) in [
            (4, None, ""),
            (3, Some("edit".into()), ""),
            (3, None, "other"),
        ] {
            assert!(
                stamp
                    .guard_if_current(opening, authority.clone(), &prompt, conversation, None)
                    .is_none()
            );
        }
        let mut edited = authority.clone();
        edited.title = Some("typed".into());
        assert!(
            stamp
                .guard_if_current(3, edited.clone(), &None, "", None)
                .is_none()
        );
        let naming = CheckoutNameSource::Template {
            base: "review".into(),
            kind: farhelm_proto::launcher::TemplateNameKind::Base,
        };
        let numbered = GuardedSubmitStamp::new(
            3,
            PreviewAuthority {
                title: Some("review".into()),
                ..authority
            },
            None,
            String::new(),
            crate::DeleteGuard::NothingAlive,
            Some(8),
        )
        .after_resume(4, Some(naming.clone()));
        edited.incarnation = 4;
        edited.title = Some("review-2".into());
        assert!(
            numbered
                .guard_if_current(3, edited.clone(), &None, "", Some(&naming))
                .is_some()
        );
        assert!(
            numbered
                .guard_if_current(3, edited, &None, "", None)
                .is_none(),
            "a hand edit retires the automatic name policy"
        );
    }

    /// A guarded answer has edit authority only for the opening and exact
    /// draft the user saw. Cancel, a changed draft, a launcher opening
    /// replaced since the render, or a busy operation must leave the question
    /// available and never initiate the transition.
    #[farhelm_testtrace::test]
    fn guarded_launcher_answers_take_only_the_current_unchanged_draft() {
        use super::*;
        use std::cell::Cell;

        std::thread_local! {
            static SLOT: Cell<Option<ConfirmSlot<LauncherYoloQuestion>>> = const { Cell::new(None) };
        }
        /// Mount the actual slot without the rest of the launcher, so each
        /// simulated event consumes the same live question state as the UI.
        fn app() -> Element {
            let slot = use_confirm_slot::<LauncherYoloQuestion, ()>();
            SLOT.with(|cell| cell.set(Some(slot)));
            rsx! {}
        }
        let question = || LauncherYoloQuestion {
            refused_key: "refused-create".into(),
            ask: crate::yolo_confirm::YoloAsk {
                host: Some(7),
                host_name: "build box".into(),
                reason: crate::yolo_confirm::YoloReason::ChosenPermission,
            },
            uncertain: false,
        };
        let refused = IntentBinding {
            prompt: Some("original prompt".into()),
            conversation: None,
            host: 7,
            incarnation: "original-installation".into(),
            cwd: "/workspace".into(),
            github_checkout: None,
            agent: LaunchIntent::Structured({
                let mut draft = LaunchDraft::new(Some(LaunchPermission::Yolo));
                draft.choose_harness(
                    LaunchHarness::Codex,
                    None,
                    String::new(),
                    ChoiceOrigin::Control,
                );
                match draft
                    .launch_intent()
                    .expect("the refused request has an agent")
                {
                    LaunchIntent::Structured(selection) => selection,
                    LaunchIntent::Command(_) => panic!("the fixture is an agent launch"),
                }
            }),
            title: "original title".into(),
            replace_source: None,
        };
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        let mut slot = SLOT.with(Cell::get).expect("the question slot mounted");
        dom.in_runtime(|| {
            let open = slot.open(question(), ());
            assert!(!take_launcher_guarded_answer(
                &mut slot,
                &open,
                true,
                1,
                1,
                &refused,
                Some(&refused)
            ));
            assert!(
                slot.peek_token() == Some(open.clone()),
                "busy leaves the opening intact"
            );
            for changed in [
                IntentBinding {
                    prompt: Some("edited before render".into()),
                    ..refused.clone()
                },
                IntentBinding {
                    conversation: Some("another conversation".into()),
                    ..refused.clone()
                },
                IntentBinding {
                    title: "edited before render".into(),
                    ..refused.clone()
                },
                IntentBinding {
                    incarnation: "replacement-installation".into(),
                    ..refused.clone()
                },
            ] {
                assert!(!take_launcher_guarded_answer(
                    &mut slot,
                    &open,
                    false,
                    1,
                    1,
                    &refused,
                    Some(&changed)
                ));
                assert!(
                    slot.peek_token() == Some(open.clone()),
                    "an edited draft leaves the opening intact"
                );
            }
            assert!(!take_launcher_guarded_answer(
                &mut slot, &open, false, 1, 1, &refused, None
            ));
            assert!(
                slot.take(&open).is_some(),
                "premise: Cancel consumed this opening"
            );
            assert!(!take_launcher_guarded_answer(
                &mut slot,
                &open,
                false,
                1,
                1,
                &refused,
                Some(&refused)
            ));
            // The same refused request asked again: an equal key, so only
            // the opening tells the stale answer apart.
            let later = slot.open(question(), ());
            assert!(!take_launcher_guarded_answer(
                &mut slot,
                &open,
                false,
                1,
                1,
                &refused,
                Some(&refused)
            ));
            assert!(
                slot.peek_token() == Some(later.clone()),
                "a stale answer cannot take a newer opening"
            );
            // Another source opened in the same launcher before this
            // answer's render was replaced: the answer belongs to the
            // earlier source and must not take the question.
            assert!(!take_launcher_guarded_answer(
                &mut slot,
                &later,
                false,
                1,
                2,
                &refused,
                Some(&refused)
            ));
            assert!(
                slot.peek_token() == Some(later.clone()),
                "a superseded launcher opening leaves the question intact"
            );
            assert!(take_launcher_guarded_answer(
                &mut slot,
                &later,
                false,
                1,
                1,
                &refused,
                Some(&refused)
            ));
            assert!(
                !slot.is_open(),
                "the admitted answer consumes its opening once"
            );
            assert!(!take_launcher_guarded_answer(
                &mut slot,
                &later,
                false,
                1,
                1,
                &refused,
                Some(&refused)
            ));
            // An earlier attempt under this key may already have started the
            // session, so the guarded answer, a new request, is refused.
            let uncertain = slot.open(
                LauncherYoloQuestion {
                    uncertain: true,
                    ..question()
                },
                (),
            );
            assert!(!take_launcher_guarded_answer(
                &mut slot,
                &uncertain,
                false,
                1,
                1,
                &refused,
                Some(&refused)
            ));
            assert!(
                slot.peek_token() == Some(uncertain),
                "an uncertain question keeps its opening"
            );
        });
    }
}

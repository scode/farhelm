//! The inline form for creating one session on a selected host.
//!
//! Intent binding and host reconciliation live beside the form because they
//! define when an idempotency key still describes the submitted create.

use dioxus::prelude::*;

use crate::api::{self, CreateAgent, create_session, mint_intent_key};
use crate::feed::{fallback_polls_now, fallback_sleep, use_feed_reader};
use crate::github_checkout::{
    DestinationDraft, GithubAttempt, GithubCheckoutRequest, GithubRepo, PreviewAuthority,
    PreviewState, RepositoryAuthority, repository_choices,
};
use crate::launch_controls::{LaunchControls, enter_choice};
use crate::ops::{ConfirmSlot, OpLock, use_confirm_slot};
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::reader::{SurfaceReader, Trigger, request_read};
use crate::{
    ApiBase, CommandLaunch, HostId, LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection,
    Session, SessionStatus,
};

use super::SharedPreferences;
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
    needs_harness: bool,
    incompatible_choice: bool,
}

impl LaunchPrerequisites {
    /// Explain the first condition the user must clear before launching.
    /// Ordering keeps transient operation locks ahead of draft corrections.
    fn refusal(&self) -> Option<&'static str> {
        if self.busy {
            Some("wait for the current operation to finish before launching")
        } else if self.saving_template {
            Some("finish or cancel saving the template before launching")
        } else if !self.host_available {
            Some("choose a connected host before launching")
        } else if self.github_search {
            Some("select a GitHub repository from search before launching")
        } else if !self.destination_ready {
            Some("wait for a current checkout preview before launching")
        } else if !self.remembered_destination_valid {
            Some(REMEMBERED_DESTINATION_CHANGED)
        } else if self.needs_harness {
            Some("choose a structured harness before launching")
        } else if self.incompatible_choice {
            Some(
                "this saved choice is no longer supported by the current catalog; choose a compatible model or effort",
            )
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

/// What a successful create hands to `on_created`: the session the helm
/// returned, and the structured launch this form SUBMITTED, if any.
///
/// Kept apart because they come from different parties. `session.launch` is
/// the supervisor's reply, which a remote host writes; the form's own
/// submission is the user's choice. Anything that remembers "what the user
/// picked" (the permission and workspace-trust mirror in `list::view`) must
/// read `submitted_launch` (SPEC.md: only explicit GUI selections shape GUI
/// defaults). `None` for command creates.
pub(crate) struct CreatedSession {
    pub(crate) session: Session,
    pub(crate) submitted_launch: Option<LaunchSelection>,
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
/// Each tab owns its own signals, so switching never copies a value from one
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

/// One edit a template makes, in the order it makes them: either a search
/// action the launcher already has (so it has exactly the effects of the
/// same action taken by hand), or a direct setting for a field no search
/// action covers (the launch kind, the command fields, a choice reset to
/// its default).
enum TemplateEdit {
    Search(crate::launch_composer::ComposerSearchResult),
    Set(Box<dyn FnOnce()>),
}

/// The launcher signals a template edits directly. All `Copy` handles,
/// bundled only so [`template_edits`] has one argument for them.
#[derive(Clone, Copy)]
struct TemplateTargets {
    launch_tab: Signal<LaunchTab>,
    structured_harness: Signal<Option<LaunchHarness>>,
    structured_model: Signal<Option<String>>,
    structured_model_raw_seed: Signal<Option<String>>,
    structured_model_edited: Signal<bool>,
    custom_model_harness: Signal<Option<LaunchHarness>>,
    composer_reset_reason: Signal<Option<String>>,
    structured_effort: Signal<Option<LaunchEffort>>,
    structured_permissions: Signal<Option<LaunchPermission>>,
    structured_permissions_is_explicit: Signal<bool>,
    structured_workspace_trust: Signal<Option<bool>>,
    structured_workspace_trust_is_explicit: Signal<bool>,
    invocation: Signal<String>,
    invocation_raw_seed: Signal<Option<String>>,
    invocation_edited: Signal<bool>,
    command_yolo: Signal<Option<bool>>,
    command_agent: Signal<Option<LaunchHarness>>,
    command_resume_on: Signal<bool>,
    command_resume: Signal<String>,
    command_resume_raw_seed: Signal<Option<String>>,
    command_resume_edited: Signal<bool>,
    chosen_host: Signal<Option<HostId>>,
    destination: Signal<DestinationDraft>,
    cwd: Signal<String>,
    cwd_raw_seed: Signal<Option<String>>,
    cwd_edited: Signal<bool>,
    title: Signal<String>,
    title_raw_seed: Signal<Option<String>>,
    title_edited: Signal<bool>,
    clone_checkout_naming: Signal<CloneCheckoutNaming>,
    template_error: Signal<Option<String>>,
    intent_key: Signal<Option<(String, IntentBinding)>>,
}

/// Capture the same raw launcher values for template application and saving.
/// Relayed editor text retains its original bytes until deliberately edited;
/// destination and name are included so saving does not grow a second reader.
fn launcher_snapshot(
    t: TemplateTargets,
    hosts: &[HostOption],
    selected: Option<HostId>,
) -> farhelm_proto::launcher::LauncherState {
    use farhelm_proto::launcher::{LauncherKind, LauncherState, TemplateDestination};
    let kind_now = match *t.launch_tab.peek() {
        LaunchTab::Agent => LauncherKind::Agent,
        LaunchTab::Command => LauncherKind::Command,
    };
    LauncherState {
        kind: Some(kind_now),
        harness: *t.structured_harness.peek(),
        model: t.structured_model.peek().as_ref().map(|model| {
            submitted_field(
                model,
                *t.structured_model_edited.peek(),
                t.structured_model_raw_seed.peek().as_deref(),
            )
        }),
        model_owner: *t.custom_model_harness.peek(),
        effort: *t.structured_effort.peek(),
        permissions: *t.structured_permissions.peek(),
        workspace_trust: *t.structured_workspace_trust.peek(),
        command: submitted_field(
            &t.invocation.peek(),
            *t.invocation_edited.peek(),
            t.invocation_raw_seed.peek().as_deref(),
        ),
        yolo: *t.command_yolo.peek(),
        command_agent: *t.command_agent.peek(),
        resume_command: t.command_resume_on.peek().then(|| {
            submitted_field(
                &t.command_resume.peek(),
                *t.command_resume_edited.peek(),
                t.command_resume_raw_seed.peek().as_deref(),
            )
        }),
        host: selected.and_then(|chosen| {
            hosts
                .iter()
                .find(|host| host.id == chosen && !host.identity_mismatch)
                .and_then(|host| host.identity.clone())
        }),
        destination: Some(match &*t.destination.peek() {
            DestinationDraft::Existing { .. } => TemplateDestination::Folder(submitted_field(
                &t.cwd.peek(),
                *t.cwd_edited.peek(),
                t.cwd_raw_seed.peek().as_deref(),
            )),
            DestinationDraft::Github { repo, .. } => {
                TemplateDestination::Github(format!("{}/{}", repo.owner, repo.name))
            }
        }),
        name: Some(effective_title(
            &t.title.peek(),
            *t.title_edited.peek(),
            t.title_raw_seed.peek().as_deref(),
            &t.destination.peek(),
            &t.clone_checkout_naming.peek(),
        )),
    }
}

/// Turn an accepted search result into the edits it makes: itself, unless it
/// is a template (`tl:name`), which becomes the edits that template contains.
///
/// SPEC.md: "Applying a template makes the edits it contains, in the same
/// way and with the same effects as making them by hand, in a fixed order",
/// all or nothing. So the template is first applied to a snapshot of the
/// launcher with the shared rule (`farhelm_proto::launcher::apply_template`),
/// which is also what the helm uses for `--template`; a refusal sets
/// `template_error` and makes no edit at all. Only then is it replayed as
/// the launcher's own actions, so remembered defaults, recent setups and a
/// pending checkout preview react exactly as they would to the same clicks.
/// `host_fixed` is Replace with, which keeps the source's host.
fn template_edits(
    result: crate::launch_composer::ComposerSearchResult,
    templates: &[farhelm_proto::launcher::LaunchTemplate],
    hosts: &[HostOption],
    catalog: &[crate::api::LaunchCatalogModel],
    host_fixed: bool,
    t: TemplateTargets,
) -> Vec<TemplateEdit> {
    use crate::launch_composer::{ComposerPermission, ComposerSearchResult};
    use farhelm_proto::launcher::{LauncherKind, TemplateDestination};
    let mut template_error = t.template_error;
    let ComposerSearchResult::Template(name) = result else {
        template_error.set(None);
        return vec![TemplateEdit::Search(result)];
    };
    let Some(template) = templates
        .iter()
        .find(|template| template.name == name)
        .cloned()
    else {
        template_error.set(Some(format!(
            "no template is named {name:?}; it may have been deleted"
        )));
        return Vec::new();
    };
    let state = launcher_snapshot(t, hosts, *t.chosen_host.peek());
    let known_hosts: Vec<String> = hosts
        .iter()
        .filter(|host| !host.identity_mismatch)
        .filter_map(|host| host.identity.clone())
        .collect();
    let context = farhelm_proto::launcher::TemplateContext {
        catalog,
        known_hosts: &known_hosts,
        host_fixed,
    };
    let applied = match farhelm_proto::launcher::apply_template(&state, &template, &context) {
        Ok(applied) => applied,
        Err(refusal) => {
            template_error.set(Some(refusal.to_string()));
            return Vec::new();
        }
    };
    // A repository the launcher cannot even parse is refused here, before
    // any edit, to keep the template all or nothing.
    let repository = match &template.fields.destination {
        Some(TemplateDestination::Github(repo)) => {
            match crate::github_checkout::GithubRepo::parse(repo) {
                Ok(repo) => Some(repo),
                Err(error) => {
                    template_error.set(Some(format!(
                        "template {name:?} was not applied: its destination {repo:?} is not a GitHub repository ({error})"
                    )));
                    return Vec::new();
                }
            }
        }
        _ => None,
    };
    template_error.set(None);
    let fields = template.fields;
    let kind = applied.kind.unwrap_or(LauncherKind::Agent);
    let mut edits = Vec::new();
    let mut t = t;
    // 1. The launch kind.
    if let Some(kind) = fields.kind {
        edits.push(TemplateEdit::Set(Box::new(move || {
            t.launch_tab.set(match kind {
                LauncherKind::Agent => LaunchTab::Agent,
                LauncherKind::Command => LaunchTab::Command,
            });
        })));
    }
    // 2. The agent type: the agent tab's, through the same action as a
    // click, or the command tab's declared type.
    if let Some(agent) = fields.agent {
        match kind {
            LauncherKind::Agent => {
                edits.push(TemplateEdit::Search(ComposerSearchResult::Harness(agent)))
            }
            LauncherKind::Command => edits.push(TemplateEdit::Set(Box::new(move || {
                t.command_agent.set(Some(agent));
            }))),
        }
    }
    // 3. The agent launch's choices.
    match fields.model {
        Some(Some(id)) => edits.push(TemplateEdit::Search(ComposerSearchResult::Model {
            id,
            harness: applied
                .harness
                .expect("a model applies only with an agent type"),
        })),
        // As choosing "harness default" by hand: no model, no owner for one,
        // and no notice about a choice that was cleared.
        Some(None) => edits.push(TemplateEdit::Set(Box::new(move || {
            t.structured_model.set(None);
            t.structured_model_raw_seed.set(None);
            t.structured_model_edited.set(false);
            t.custom_model_harness.set(None);
            t.composer_reset_reason.set(None);
        }))),
        None => {}
    }
    match fields.effort {
        Some(Some(effort)) => {
            edits.push(TemplateEdit::Search(ComposerSearchResult::Effort(effort)))
        }
        Some(None) => edits.push(TemplateEdit::Set(Box::new(move || {
            t.structured_effort.set(None)
        }))),
        None => {}
    }
    match fields.permissions {
        Some(Some(LaunchPermission::Yolo)) => edits.push(TemplateEdit::Search(
            ComposerSearchResult::Permissions(ComposerPermission::Yolo),
        )),
        Some(None) => edits.push(TemplateEdit::Search(ComposerSearchResult::Permissions(
            ComposerPermission::Default,
        ))),
        Some(Some(permission)) => edits.push(TemplateEdit::Set(Box::new(move || {
            t.structured_permissions.set(Some(permission));
            t.structured_permissions_is_explicit.set(true);
        }))),
        None => {}
    }
    match fields.workspace_trust {
        Some(Some(trust)) => edits.push(TemplateEdit::Search(ComposerSearchResult::Trust(trust))),
        Some(None) => edits.push(TemplateEdit::Set(Box::new(move || {
            t.structured_workspace_trust.set(None);
            t.structured_workspace_trust_is_explicit.set(true);
        }))),
        None => {}
    }
    // 4. The command launch's fields, typed text the user owns from here.
    if let Some(command) = fields.command {
        edits.push(TemplateEdit::Set(Box::new(move || {
            t.invocation.set(command);
            t.invocation_raw_seed.set(None);
            t.invocation_edited.set(true);
        })));
    }
    if let Some(yolo) = fields.yolo {
        edits.push(TemplateEdit::Set(Box::new(move || {
            t.command_yolo.set(Some(yolo))
        })));
    }
    if let Some(resume) = fields.resume_command {
        edits.push(TemplateEdit::Set(Box::new(move || {
            t.command_resume_on.set(resume.is_some());
            t.command_resume.set(resume.unwrap_or_default());
            t.command_resume_raw_seed.set(None);
            t.command_resume_edited.set(true);
        })));
    }
    // 5. The destination and the name, through their own actions.
    if let Some(identity) = &fields.host
        && let Some(host) = template_host(hosts, identity)
    {
        edits.push(TemplateEdit::Search(ComposerSearchResult::Host(
            crate::launch_composer::ComposerHost {
                id: host.id,
                name: host.name.clone(),
                label: host.label(),
                local: host.local,
            },
        )));
    }
    match (fields.destination, repository) {
        (Some(TemplateDestination::Folder(folder)), _) => {
            edits.push(TemplateEdit::Search(ComposerSearchResult::UsePath(folder)));
        }
        (Some(TemplateDestination::Github(_)), Some(repo)) => {
            edits.push(TemplateEdit::Search(ComposerSearchResult::Github(repo)));
        }
        _ => {}
    }
    if let Some(name) = fields.name {
        edits.push(TemplateEdit::Search(ComposerSearchResult::Name(name)));
    }
    edits.push(TemplateEdit::Set(Box::new(move || t.intent_key.set(None))));
    edits
}

/// Apply a clicked or keyboard-selected search result without launching.
///
/// Search is only a picker. Keeping its result application in one helper
/// makes Enter and pointer activation replace the same fields and clear the
/// same idempotency binding. Name and host actions therefore have the same
/// no-launch contract as other results. A returned path means this was the
/// explicit browse action; the caller starts the shared guarded request after
/// closing the search surface.
#[expect(
    clippy::too_many_arguments,
    reason = "the signals are independent reactive ownership handles; bundling them would obscure which draft fields a search action may replace"
)]
fn apply_composer_search_result(
    result: crate::launch_composer::ComposerSearchResult,
    mut title: Signal<String>,
    mut title_edited: Signal<bool>,
    mut chosen_host: Signal<Option<HostId>>,
    hosts: &[HostOption],
    mut clone_host_state: Signal<CloneHostState>,
    history_target: Option<CreateTarget>,
    mut live_destination: Signal<Option<CreateTarget>>,
    mut remembered_destination: Signal<Option<CreateTarget>>,
    history_activation_attempts: Signal<u64>,
    mut destination_draft: Signal<DestinationDraft>,
    mut preview_revision: Signal<u64>,
    mut cwd: Signal<String>,
    mut cwd_raw_seed: Signal<Option<String>>,
    mut cwd_edited: Signal<bool>,
    mut folder_is_explicit: Signal<bool>,
    mut launch_tab: Signal<LaunchTab>,
    mut structured_harness: Signal<Option<LaunchHarness>>,
    mut structured_model: Signal<Option<String>>,
    mut structured_model_raw_seed: Signal<Option<String>>,
    mut structured_model_edited: Signal<bool>,
    mut custom_model_harness: Signal<Option<LaunchHarness>>,
    mut structured_effort: Signal<Option<LaunchEffort>>,
    mut structured_permissions: Signal<Option<LaunchPermission>>,
    mut structured_permissions_is_explicit: Signal<bool>,
    mut structured_workspace_trust: Signal<Option<bool>>,
    mut structured_workspace_trust_is_explicit: Signal<bool>,
    mut composer_reset_reason: Signal<Option<String>>,
    // `None` while the catalog read is pending or failed; see
    // `launch_composer::selection_fits_catalog`.
    catalog_answer: Option<&[crate::api::LaunchCatalogModel]>,
    mut intent_key: Signal<Option<(String, IntentBinding)>>,
) -> Option<String> {
    let catalog = catalog_answer.unwrap_or_default();
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
        // A template is expanded into the edits it contains before it gets
        // here (`template_edits`), and none of those edits is a template.
        ComposerSearchResult::Template(_) => {
            unreachable!("template_edits expands every template result")
        }
        ComposerSearchResult::Name(name) => {
            title.set(name);
            // A cloned title may have an escaped display seed. This action
            // is the user's new text, so submit and checkout preview must
            // read it rather than replay the clone's raw title.
            title_edited.set(true);
        }
        ComposerSearchResult::Host(host) => {
            // A host action has the same authority effects as the selector:
            // it takes over clone defaults and retires the old destination
            // before a queued history callback or submit can observe it.
            if hosts.iter().any(|offered| offered.id == host.id) {
                chosen_host.set(Some(host.id));
                live_destination.set(self::history_target(hosts, Some(host.id)));
                remembered_destination.set(None);
                clone_host_state.set(CloneHostState::UserTookOver);
            }
        }
        ComposerSearchResult::Github(repo) => {
            remembered_destination.set(None);
            destination_draft.set(DestinationDraft::github(repo));
            preview_revision.with_mut(|value| {
                *value = value.checked_add(1).expect("preview revision exhausted")
            });
        }
        crate::launch_composer::ComposerSearchResult::UsePath(folder)
        | crate::launch_composer::ComposerSearchResult::Folder(folder) => {
            select_existing_directory(
                &mut destination_draft,
                &mut cwd,
                &mut cwd_raw_seed,
                &mut cwd_edited,
                &mut folder_is_explicit,
                &folder,
            );
        }
        crate::launch_composer::ComposerSearchResult::BrowsePath(folder) => {
            // Query-path actions are treated like other relayed path choices:
            // the escaped field remains reviewable, while the browse request
            // and an untouched later create retain the exact requested bytes.
            select_existing_directory(
                &mut destination_draft,
                &mut cwd,
                &mut cwd_raw_seed,
                &mut cwd_edited,
                &mut folder_is_explicit,
                &folder,
            );
            intent_key.set(None);
            return Some(folder);
        }
        crate::launch_composer::ComposerSearchResult::Harness(harness) => {
            launch_tab.set(LaunchTab::Agent);
            let before = LaunchSelection {
                harness: structured_harness().unwrap_or(harness),
                model: structured_model(),
                effort: structured_effort(),
                permissions: structured_permissions(),
                workspace_trust: structured_workspace_trust(),
            };
            let (selection, owner) = crate::launch_composer::reconcile_harness_for_catalog_read(
                before.clone(),
                *custom_model_harness.peek(),
                harness,
                catalog_answer,
            );
            composer_reset_reason.set(draft_reconciliation_reason(
                &before,
                &selection,
                structured_permissions_is_explicit(),
            ));
            structured_harness.set(Some(selection.harness));
            structured_model_raw_seed.set(selection.model.clone());
            structured_model_edited.set(false);
            structured_model.set(selection.model);
            structured_effort.set(selection.effort);
            structured_permissions.set(selection.permissions);
            structured_workspace_trust.set(selection.workspace_trust);
            custom_model_harness.set(owner);
        }
        crate::launch_composer::ComposerSearchResult::Model { id, harness } => {
            // A model carries its harness ownership, so selecting it is also
            // an explicit return to structured mode rather than leaving a
            // structured draft hidden behind command controls.
            launch_tab.set(LaunchTab::Agent);
            let before = LaunchSelection {
                harness: structured_harness().unwrap_or(harness),
                model: structured_model(),
                effort: structured_effort(),
                permissions: structured_permissions(),
                workspace_trust: structured_workspace_trust(),
            };
            // Retain the source harness until reconciliation has removed its
            // default permission; the selected model belongs to the target.
            let selection = LaunchSelection {
                model: Some(id),
                ..before.clone()
            };
            let (selection, owner) = crate::launch_composer::reconcile_harness_for_catalog_read(
                selection,
                Some(harness),
                harness,
                catalog_answer,
            );
            composer_reset_reason.set(draft_reconciliation_reason(
                &before,
                &selection,
                structured_permissions_is_explicit(),
            ));
            structured_harness.set(Some(selection.harness));
            structured_model_raw_seed.set(selection.model.clone());
            structured_model_edited.set(false);
            structured_model.set(selection.model);
            structured_effort.set(selection.effort);
            structured_permissions.set(selection.permissions);
            structured_workspace_trust.set(selection.workspace_trust);
            custom_model_harness.set(owner);
        }
        crate::launch_composer::ComposerSearchResult::Effort(effort) => {
            // Only the effort moves. Harness and model stay, so no
            // reconciliation runs and `composer_reset_reason` is deliberately
            // left as it was — the same as clicking the effort segment, which
            // is what this result is a keyboard spelling of. The shared
            // `intent_key.set(None)` below still applies: a changed effort is
            // a changed create.
            structured_effort.set(Some(effort));
        }
        crate::launch_composer::ComposerSearchResult::Trust(trust) => {
            structured_workspace_trust.set(Some(trust));
            structured_workspace_trust_is_explicit.set(true);
        }
        crate::launch_composer::ComposerSearchResult::Permissions(permission) => {
            match permission {
                crate::launch_composer::ComposerPermission::Default => {
                    structured_permissions.set(None);
                }
                crate::launch_composer::ComposerPermission::Yolo => {
                    structured_permissions.set(Some(LaunchPermission::Yolo));
                }
            }
            structured_permissions_is_explicit.set(true);
        }
        crate::launch_composer::ComposerSearchResult::Recent(entry) => {
            launch_tab.set(LaunchTab::Agent);
            composer_reset_reason.set(None);
            let mut selection = crate::launch_composer::select_recent(&entry);
            selection.permissions = crate::launch_composer::normalized_permissions(
                selection.harness,
                selection.permissions,
            );
            selection.workspace_trust = crate::launch_composer::normalized_workspace_trust(
                selection.harness,
                selection.workspace_trust,
            );
            let owner = crate::launch_composer::custom_model_owner(&selection, catalog);
            structured_harness.set(Some(selection.harness));
            structured_model_raw_seed.set(selection.model.clone());
            structured_model_edited.set(false);
            structured_model.set(selection.model);
            structured_effort.set(selection.effort);
            structured_permissions.set(selection.permissions);
            structured_workspace_trust.set(selection.workspace_trust);
            structured_workspace_trust_is_explicit.set(true);
            // A search-applied recent replaces the whole draft with a real
            // choice, not a passive default — see
            // `structured_permissions_is_explicit`'s own doc.
            structured_permissions_is_explicit.set(true);
            custom_model_harness.set(owner);
            if let Some(repo) = entry.github_repo {
                remembered_destination.set(None);
                destination_draft.set(DestinationDraft::github(repo));
                preview_revision.with_mut(|value| {
                    *value = value.checked_add(1).expect("preview revision exhausted")
                });
            } else {
                select_existing_directory(
                    &mut destination_draft,
                    &mut cwd,
                    &mut cwd_raw_seed,
                    &mut cwd_edited,
                    &mut folder_is_explicit,
                    &entry.cwd,
                );
            }
        }
    }
    intent_key.set(None);
    None
}

/// Every explicit path choice leaves fresh-checkout mode. Keep the raw path
/// authority and its escaped editor spelling in the same synchronous action,
/// before a queued submit or directory callback can observe the choice.
fn select_existing_directory(
    destination: &mut Signal<DestinationDraft>,
    cwd: &mut Signal<String>,
    raw_seed: &mut Signal<Option<String>>,
    edited: &mut Signal<bool>,
    folder_is_explicit: &mut Signal<bool>,
    path: &str,
) {
    destination.set(DestinationDraft::Existing {
        cwd: path.to_string(),
    });
    reseed_cloned_field(cwd, raw_seed, edited, path);
    folder_is_explicit.set(true);
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

/// Submit the launcher's form, as its Launch button would.
///
/// Choice shortcuts apply their state before this call, and YOLO answers
/// take their confirmation before it. Native submit buttons cannot express
/// either ordering. `requestSubmit` reaches the same `onsubmit` as Launch,
/// which reads the live draft and retains authority over admission.
fn resubmit_composer() {
    document::eval(
        "document.querySelector('.create-session-form[role=\"dialog\"]')?.requestSubmit()",
    );
}

/// One opening of the launcher's YOLO question: the key its
/// [`ConfirmSlot`] is open for.
///
/// The question belongs to the launch the helm refused, named by that
/// request's intent key (`refused_key`), and `opening` counts openings, so an
/// answer drawn for an earlier question can never take a later one. An answer
/// records consent only by taking the question its handler was drawn for:
/// Cancel followed by either answer in one event burst (all three drawn
/// before the render that removes them) finds the slot empty and records
/// nothing. `ask` is what the question shows; the slot holds nothing else.
#[derive(Clone, PartialEq)]
struct LauncherYoloQuestion {
    opening: u64,
    refused_key: String,
    ask: crate::yolo_confirm::YoloAsk,
}

/// Answer the launcher's YOLO question `question` (the opening this answer's
/// handler was drawn for). When it is still open: take it, record consent
/// for its refused request in `confirmed_key`, and for "don't ask again"
/// (`stop_asking`) also in `stop_asking_for`, and return `true`, after which
/// the caller resubmits the form. Otherwise change nothing and return
/// `false`: a stale answer leaves the form unauthorized and must not submit.
///
/// The one-off answer also clears `stop_asking_for`, so it can never mark the
/// host. Consent stays recorded under the refused key after the submit, which
/// is what lets a retry of the same request (a confirmed create whose reply
/// was lost) keep it; see `yolo_confirmed_key`.
fn answer_launcher_yolo(
    slot: &mut ConfirmSlot<LauncherYoloQuestion>,
    question: &LauncherYoloQuestion,
    stop_asking: bool,
    confirmed_key: &mut Option<String>,
    stop_asking_for: &mut Option<LauncherYoloQuestion>,
) -> bool {
    if slot.take(question).is_none() {
        return false;
    }
    *confirmed_key = Some(question.refused_key.clone());
    *stop_asking_for = stop_asking.then(|| question.clone());
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
    /// Fresh requests carry the exact accepted preview as part of the key's
    /// identity. Existing requests leave this absent and keep their old body.
    github_checkout: Option<GithubCheckoutRequest>,
    /// What this create launches — see [`LaunchIntent`]. Switching between
    /// the two modes is a different intended create, which is why the mode
    /// lives inside the binding rather than beside it.
    agent: LaunchIntent,
    title: String,
    /// `Some(source id)` when this binding is a "replace with", carried
    /// straight from the mounted [`CreatePrefill::replace_source`] — `None`
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

impl IntentBinding {
    /// The binding for a submit, or `None` when there is no host to create
    /// on — the one case a submit is refused locally rather than sent.
    fn of(
        selected: Option<HostId>,
        hosts: &[HostOption],
        cwd: String,
        agent: LaunchIntent,
        title: String,
        replace_source: Option<String>,
    ) -> Option<IntentBinding> {
        let host = hosts.iter().find(|host| Some(host.id) == selected)?;
        Some(IntentBinding {
            host: host.id,
            incarnation: host.incarnation.clone(),
            cwd,
            github_checkout: None,
            agent,
            title,
            replace_source,
        })
    }
}

/// Reconciliation follows user intent and installation, not the latest config
/// or connection counter. The retained body still carries its original preview
/// and incarnation for the helm's authenticated reconciliation path.
fn same_fresh_intent(
    accepted: &IntentBinding,
    current: &IntentBinding,
    repo: &GithubRepo,
    installation: &str,
) -> bool {
    accepted.host == current.host
        && accepted.agent == current.agent
        && accepted.title == current.title
        && accepted.replace_source == current.replace_source
        && accepted.github_checkout.as_ref().is_some_and(|checkout| {
            checkout.repo == repo.identifier()
                && checkout.preview.installation_identity == installation
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
                        result.set(Some((requested, history)));
                    }
                    true
                }
                Err(_) => false,
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

/// Give the shared composer search focus at an explicit interaction boundary.
///
/// Callers invoke this only when the dialog mounts, a mode button is pressed,
/// or a search result is accepted. Ordinary rerenders must never reclaim focus
/// from a field the user deliberately entered.
fn focus_composer_surface() {
    document::eval(
        r#"(() => {
            const dialog = document.querySelector('.create-session-form[role="dialog"]');
            if (!dialog) return;
            const target = dialog.querySelector('.launch-composer-search input:not([disabled])');
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
    /// `Some(source id)` makes this a Replace-with prefill. Its initial
    /// destination stays the source folder instead of Clone's fresh checkout.
    /// Host reconciliation and launch settings still seed identically; submit
    /// uses the replace endpoint and refuses a drift from the source host.
    /// The launch button reads "replace"; [`IntentBinding::replace_source`]
    /// carries the source through asynchronous submission. Prefill generations
    /// and remembered permissions still follow the same reconciliation rules.
    pub(super) replace_source: Option<String>,
    /// The "replace with" source's status and terminal tab count when the
    /// launcher opened, `None` for a plain clone.
    ///
    /// Only the last fallback: `ListView` hands the form the source's row
    /// from the live listing whenever the listing has it, then the last state
    /// a listing showed, and this snapshot only before any listing has shown
    /// the row (a header "replace with" on a session the sidebar's filter
    /// hides, say). Either way the launcher's warning and the
    /// precondition its replace sends come from the same value; see
    /// `CreateSessionForm`'s `replace_source_state`.
    pub(super) replace_source_opened: Option<(SessionStatus, usize)>,
}

/// Build the prefill a clone click seeds the create form with, from the
/// clicked row's own `Session`.
///
/// Kept as a pure mapping — apart from `CreateSessionForm` and from whatever
/// mints `generation` (`list::view::ListView`, once per clone click) — so
/// what a clone carries is checkable without mounting a component.
///
/// Always leaves [`CreatePrefill::replace_source`] `None`: this function
/// builds a CLONE's prefill specifically, and `list::view::ListView`'s own
/// "replace with" handler is what sets that field afterward on the value
/// this returns, since only the caller knows which row is being
/// replaced-with rather than merely cloned.
pub(super) fn prefill_from(session: &Session, generation: u64) -> CreatePrefill {
    CreatePrefill {
        generation,
        host: session.host,
        host_identity: session.host_identity.clone(),
        cwd: session.cwd.clone(),
        repo: session
            .working_copy
            .as_ref()
            .map(|checkout| checkout.repo.clone())
            .or_else(|| session.github_repo.clone()),
        title: session.title.clone(),
        invocation: session.invocation.clone(),
        launch: session.agent_selection().cloned(),
        command: match &session.launch {
            Some(crate::SessionLaunch::Command(command)) => Some(command.clone()),
            Some(crate::SessionLaunch::Agent { .. })
            | Some(crate::SessionLaunch::Legacy { .. })
            | None => None,
        },
        replace_source: None,
        replace_source_opened: None,
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
/// lacks the row (see [`CreatePrefill::replace_source_opened`]).
pub(super) fn mark_replace_with(prefill: &mut CreatePrefill, session: &Session) {
    prefill.replace_source = Some(session.id.clone());
    prefill.replace_source_opened = Some((session.status.clone(), session.tabs.len()));
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
/// [`reseed_cloned_field`] is the seeding half (item2-review2.md F5).
pub(crate) fn submitted_field(text: &str, edited: bool, seed: Option<&str>) -> String {
    match seed {
        Some(seed) if !edited => seed.to_string(),
        _ => text.to_string(),
    }
}

/// Seed one of the create form's peer-relayed text fields (working
/// directory, invocation, title) from a clone's raw value: shown escaped
/// (`peer::display_peer`), with the exact raw bytes and a cleared edited
/// flag recorded alongside so an untouched submit reads back the ORIGINAL
/// bytes rather than the escaped spelling (item2-review2.md F5;
/// [`submitted_field`] is the read-back half).
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

/// Whether the title a Clone or Replace with carried in stops counting as a
/// name once the destination is a fresh GitHub checkout.
///
/// Clone and Replace with copy the source's title into the name field, and
/// for an existing-folder launch that copy is what the new session should be
/// called. A fresh checkout is different: its directory is named after the
/// title, and SPEC.md (Fresh GitHub checkouts) refuses an explicit name that
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

/// A Clone's untouched checkout name, scoped to the current host and repository.
///
/// Only the UI's default may advance after an occupied-name refusal. Explicit
/// edits, Replace with, and a different repository keep their existing rules.
/// The bound prevents a peer from driving an unlimited sequence of previews.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CloneCheckoutNaming {
    source_repo: Option<GithubRepo>,
    target: Option<(Option<CreateTarget>, Option<GithubRepo>)>,
    suffix: u32,
}

impl CloneCheckoutNaming {
    /// Occupancy belongs to one host installation and repository. Returning
    /// to a prior target starts its search over, rather than caching names.
    fn retarget(&mut self, host: Option<CreateTarget>, repo: Option<GithubRepo>) {
        let target = Some((host, repo));
        if self.target != target {
            self.target = target;
            self.suffix = 0;
        }
    }

    /// Preserve the source title's raw spelling until the person edits it.
    /// An empty source title remains an unnamed checkout, as ordinary New does.
    fn default_title(
        &self,
        edited: bool,
        seed: Option<&str>,
        repo: Option<&GithubRepo>,
    ) -> Option<String> {
        if edited || self.source_repo.is_none() || self.source_repo.as_ref() != repo {
            return None;
        }
        let seed = seed.filter(|seed| !seed.is_empty())?;
        Some(if self.suffix == 0 {
            format!("{seed}-clone")
        } else {
            format!("{seed}-clone-{}", self.suffix)
        })
    }

    /// Try the next suffix only after the current authoritative preview says
    /// occupied. An unresolved create keeps its name for reconciliation: its
    /// own accepted directory may be the collision. At the cap, retain the
    /// visible conflict for a manual edit.
    fn advance(&mut self, reconciling: bool) -> bool {
        if reconciling || self.suffix >= 50 {
            return false;
        }
        self.suffix = if self.suffix == 0 { 2 } else { self.suffix + 1 };
        true
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
            format!("use \"{}\" as the session's name", display_peer(name))
        }
        ComposerSearchResult::Host(_) => "run the session on this host".to_string(),
        ComposerSearchResult::UsePath(_) | ComposerSearchResult::Folder(_) => {
            "use this folder: the session will start here".to_string()
        }
        ComposerSearchResult::BrowsePath(_) => "list the folders under this path".to_string(),
        ComposerSearchResult::Harness(_) => "use this agent for the session".to_string(),
        ComposerSearchResult::Template(_) => {
            "apply this template's settings to the draft".to_string()
        }
        ComposerSearchResult::Github(_) => {
            "start in a fresh checkout of this repository".to_string()
        }
        ComposerSearchResult::Model { .. } => "use this model for the session".to_string(),
        ComposerSearchResult::Effort(_) => "use this reasoning effort for the session".to_string(),
        ComposerSearchResult::Permissions(ComposerPermission::Default) => {
            "use the agent's default permissions".to_string()
        }
        ComposerSearchResult::Permissions(ComposerPermission::Yolo) => {
            crate::launch_composer::permission_tooltip(LaunchPermission::Yolo).to_string()
        }
        ComposerSearchResult::Trust(value) => {
            crate::launch_composer::workspace_trust_tooltip(harness, Some(*value)).to_string()
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

/// Read the name the launcher actually offers, including Clone's free suffix.
/// Template snapshots and launches must share this decision: saving the source
/// title instead would pin a name that this checkout deliberately avoids.
fn effective_title(
    text: &str,
    edited: bool,
    seed: Option<&str>,
    destination: &DestinationDraft,
    naming: &CloneCheckoutNaming,
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
/// `submitting` is owned by the CALLER (`ListView`), not this component:
/// `ListView`'s own "new session" toggle button needs to see it too, so it
/// can refuse to unmount this form while a create is still in flight —
/// dropping this component mid-`spawn` would strand the POST's eventual
/// response with nothing left to act on it. Lifting the flag up is
/// simpler than trying to keep a detached task meaningful after the fact.
///
/// `on_created` fires only on a successful POST, with the newly created
/// `Session` from the response body and the launch this form submitted
/// ([`CreatedSession`]); `ListView` uses that to close the
/// form and select the new session in the adjacent pane, whose terminal
/// mounts immediately (SPEC.md: "creation launches the agent; you type
/// your first prompt into its terminal") — the sidebar itself stays
/// mounted throughout. On failure the form stays mounted with its values
/// untouched and the refusal rendered beside Launch — the fields are
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
/// identical prefill shape, marked only by `CreatePrefill::replace_source`;
/// nothing in this section, or in the reseed effect below, branches on that
/// field — the two verbs differ only past submit (see `IntentBinding::
/// replace_source`), which is the whole point of routing both through one
/// prefill mechanism instead of a second one. The reseed effect replaces the
/// whole agent draft for each source generation. The launch is independent of
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
    /// The installation currently selected for the create, for host safety
    /// checks.
    create_target: Signal<Option<CreateTarget>>,
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
    /// effect is what turns a new
    /// generation into field values.
    prefill: Option<CreatePrefill>,
    /// One accepted switcher action for an ordinary New draft. Applied once
    /// after mount seeding (templates also wait for asynchronous inputs),
    /// through the same search-accept path as a keyboard or mouse choice. This is not a clone
    /// prefill: remembered permissions and New's destination still seed normally.
    initial_action: Option<crate::launch_composer::ComposerSearchResult>,
    /// For "replace with", the source session's status and terminal tab
    /// count as the page currently knows them: its row in the live listing,
    /// else the last state a listing showed for it, else
    /// [`CreatePrefill::replace_source_opened`].
    /// `None` for a create or a plain clone.
    ///
    /// SPEC.md "Replace with": the launcher's replace button is the
    /// confirmation, so while this shows anything alive the launcher draws
    /// Replace's warning, and the replace it sends carries the precondition
    /// matching that same value (`status::delete_guard`). Both are derived
    /// from this one prop in the same render, so the request never authorizes
    /// more than the text on screen said.
    replace_source_state: Option<(SessionStatus, usize)>,
    /// Discard this draft without creating a session.
    on_cancel: EventHandler<()>,
    on_created: EventHandler<CreatedSession>,
    /// A template save closes the launcher and hands its stored snapshot to the editor.
    on_template_saved: EventHandler<farhelm_proto::launcher::LaunchTemplate>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    // The helm-wide preference row (`PreferencesGate`'s seed): read here
    // only to seed a fresh no-prefill dialog's permissions segment and to
    // answer "reset choices" — see `initial_structured_permissions`. This
    // form never WRITES through this signal; the helm sets
    // `remembered_permissions` as a side effect of a successful launch, and
    // `list::view`'s `on_created` handler mirrors this client's own result
    // into it afterward.
    let preferences = use_context::<SharedPreferences>();
    let launch_catalog_base = base.clone();
    let mut launch_catalog = use_resource(move || {
        let base = launch_catalog_base.clone();
        async move { api::fetch_launch_catalog(&base).await }
    });
    // The helm's launch templates, read once per dialog, for `tl:` search. A
    // failed read offers none; nothing else in the launcher depends on them.
    let templates_base = base.clone();
    // Re-read whenever the Templates dialog closes (see `TemplatesRevision`);
    // absent outside the list view, where nothing can change them.
    let templates_revision = try_use_context::<super::templates::TemplatesRevision>();
    let launch_templates = use_resource(move || {
        let base = templates_base.clone();
        let _revision = templates_revision.map(|revision| (revision.0)());
        async move { api::fetch_templates(&base).await.unwrap_or_default() }
    });
    // Why the last template accepted from search was not applied, shown
    // under the search box until the next accepted result.
    let template_error = use_signal(|| None::<String>);
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
    let destination_seed = initial_cwd.clone().unwrap_or_else(|| "~".into());
    let mut destination_draft = use_signal(move || DestinationDraft::Existing {
        cwd: destination_seed,
    });
    let mut clone_checkout_naming = use_signal(CloneCheckoutNaming::default);
    let mut github_attempt = use_signal(|| None::<(IntentBinding, GithubAttempt)>);
    // A Replace-with deletes its source, whose notice the session list shows.
    let delete_notice = use_context::<super::DeleteNotice>();
    let mut preview_generation = use_signal(|| 0_u64);
    let mut preview_revision = use_signal(|| 0_u64);
    let mut observed_checkout_revision = use_signal(|| 0_i64);
    let mut live_preview_authority = use_signal(|| None::<PreviewAuthority>);
    let mut invocation = use_signal(String::new);
    // The command launch's other fields (SPEC.md, Creation): the YOLO
    // assertion, unanswered until the user picks; the declared agent type;
    // and Resume with its own command.
    let mut command_yolo = use_signal(|| None::<bool>);
    let mut command_agent = use_signal(|| None::<LaunchHarness>);
    let mut command_resume_on = use_signal(|| false);
    let mut command_resume = use_signal(String::new);
    // A cloned resume command is peer-relayed text like the command itself:
    // shown escaped, with its raw bytes kept to send back untouched (see
    // `reseed_cloned_field` and `submitted_field`).
    let mut command_resume_raw_seed = use_signal(|| None::<String>);
    let mut command_resume_edited = use_signal(|| false);
    let mut title = use_signal(String::new);
    let mut launch_tab = use_signal(|| LaunchTab::Agent);
    let mut structured_harness = use_signal(|| None::<LaunchHarness>);
    let mut structured_model = use_signal(|| None::<String>);
    // A restored custom id is peer text even though it is stored in a launch
    // selection. Keep its raw bytes for submission while showing an escaped
    // spelling until the person deliberately edits the field.
    let mut structured_model_raw_seed = use_signal(|| None::<String>);
    let mut structured_model_edited = use_signal(|| false);
    let mut custom_model_harness = use_signal(|| None::<LaunchHarness>);
    let mut structured_effort = use_signal(|| None::<LaunchEffort>);
    // Seeded from the helm's remembered permissions mode ONLY when this
    // mount has no prefill at all (SPEC.md's launch-composer carve-out): a
    // clone/replace prefill's own reseed effect below (`prefill.launch`)
    // unconditionally overwrites this seed the moment it runs, so a
    // prefilled mount briefly starting at `None` here is never visible —
    // see `initial_structured_permissions`'s own doc for why the precedence
    // is a call-site guard rather than logic inside that function.
    let mut structured_permissions = use_signal(|| {
        if prefill.is_none() {
            initial_structured_permissions(&preferences.0.peek())
        } else {
            None
        }
    });
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
    // permissions needs this second signal.
    let mut structured_permissions_is_explicit = use_signal(|| false);
    // The helm records only an explicit choice from a supported harness.
    // Seeding the draft does not send it through unsupported harnesses.
    let mut structured_workspace_trust = use_signal(|| {
        prefill
            .is_none()
            .then(|| preferences.0.peek().remembered_workspace_trust)
            .flatten()
    });
    // A passive remembered default should not hide recents that made a
    // different explicit choice. Search, controls, clone, and recents do.
    let mut structured_workspace_trust_is_explicit = use_signal(|| false);
    // Compatibility clears are intentional, but defaults must never make an
    // earlier explicit choice vanish without telling the person what changed.
    let mut composer_reset_reason = use_signal(|| None::<String>);
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
    // What each of the three text fields above was SEEDED from, raw, and
    // whether the user has typed in it since (item2-review2.md F5): a
    // clone's directory, invocation and title are peer-relayed text going
    // into an editable control, and an untouched field must submit the
    // ORIGINAL bytes rather than the escaped spelling `cwd`/`invocation`/
    // `title` display while the clone is on screen (see the reseed effect
    // below for where the escaping is applied, and [`submitted_field`] for
    // the read-back half these are fed into at submit time). `None` seeds
    // mean "never clone-seeded", which is the ordinary blank-create case:
    // there the field's own text already IS the value to submit, since
    // nothing relayed it from a peer.
    let cwd_initial_raw_seed = initial_cwd.clone();
    let mut cwd_raw_seed = use_signal(move || cwd_initial_raw_seed);
    let mut cwd_edited = use_signal(|| false);
    // Text ownership and deliberate placement are different: a picked path
    // keeps a raw seed, but is still pre-checked when saving a template.
    let mut folder_is_explicit = use_signal(|| false);
    // Keep the host's readable label with the same immutable choice snapshot;
    // a later host pick must not relabel the identity this panel will save.
    let mut template_panel = use_signal(|| None::<(super::save_template::Candidate, String)>);
    let template_saving = use_signal(|| false);
    let mut invocation_raw_seed = use_signal(|| None::<String>);
    let mut invocation_edited = use_signal(|| false);
    let mut title_raw_seed = use_signal(|| None::<String>);
    let mut title_edited = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    // An untouched dialog is allowed to be incomplete. Once attempted, its
    // prerequisite reason follows the current draft and disappears when ready;
    // request failures retain the helm's own words until the next attempt.
    let mut launch_attempted = use_signal(|| false);
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
    // The question itself is a `ConfirmSlot` (`LauncherYoloQuestion`): an
    // answer records consent only by taking the question it was drawn for,
    // then resubmits the form, so an answer queued behind Cancel records
    // nothing and submits nothing (`answer_launcher_yolo`).
    //
    // "Start, and don't ask again on this host" adds a request to mark the
    // host safe first, held the same way: the question it answered (and so
    // its refused key), set by that button's click just before the form
    // resubmits, and taken by that one submit, so it can never ride along
    // with a later Launch or "start anyway". `yolo_error` is that first
    // step's failure; the question comes back up with it.
    let mut yolo_refusal = use_confirm_slot::<LauncherYoloQuestion, ()>();
    let mut yolo_openings = use_signal(|| 0u64);
    let mut yolo_confirmed_key = use_signal(|| None::<String>);
    let mut yolo_stop_asking = use_signal(|| None::<LauncherYoloQuestion>);
    let mut yolo_error = use_signal(|| None::<String>);
    // Open the question for `refused_key` as a new opening.
    let mut open_yolo_question = move |refused_key: String, ask: crate::yolo_confirm::YoloAsk| {
        let opening = *yolo_openings.peek() + 1;
        yolo_openings.set(opening);
        yolo_refusal.open(
            LauncherYoloQuestion {
                opening,
                refused_key,
                ask,
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
    let mut clone_host_state = use_signal(|| CloneHostState::Unconfirmable);
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
            // Worded for the SOURCE session rather than "the session you
            // cloned": a replace-with prefill takes this same path, and its
            // user did not clone anything.
            CloneHostState::Unconfirmable => Some(
                "the source session reports a different installation now, so its host was \
                 not carried over here — check the host below",
            ),
            CloneHostState::Waiting | CloneHostState::Bound | CloneHostState::UserTookOver => None,
        });
    // Taken before the reseed effect below moves the prop into its own
    // `move` closure: the submit handler needs `replace_source` and `host`
    // long after that effect has run (one owned copy, consumed by its
    // closure), and the launch button's verb only needs to know whether
    // this is a replace-with at all — a `bool`, derived here rather than
    // a second clone of the whole prefill.
    let prefill_for_submit = prefill.clone();
    let is_replace_with = prefill
        .as_ref()
        .is_some_and(|prefill| prefill.replace_source.is_some());
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
    let history_reader = use_signal(SurfaceReader::default);
    let launch_history = use_signal(|| None::<(CreateTarget, api::LaunchHistory)>);
    let history_feed_base = base.clone();
    use_feed_reader(move || {
        request_launch_history(
            history_feed_base.clone(),
            create_target,
            history_reader,
            launch_history,
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

    // A host installation change rotates the idempotency key and any
    // host-specific refusal; the launch draft survives it.
    let mut bound_target = use_signal(|| None::<CreateTarget>);
    // Which prefill GENERATION (`CreatePrefill`) this form has already
    // applied. Compared by generation rather than by mere presence in the
    // effect below, because `prefill` stays populated at its latest
    // generation for as long as this form is open, so presence alone would
    // reseed on every unrelated rerun of that effect (a host reconnect, a
    // catalog refresh) for as long as a clone is on screen.
    let mut prefill_applied = use_signal(|| None::<u64>);
    // Initial search acceptance follows this effect, so mount-time host and
    // clone reconciliation cannot overwrite the action it applies.
    let mut mount_seeded = use_signal(|| false);
    let mut pending_initial_action = use_signal(move || initial_action);
    // Cloned rather than borrowed into the effect below: `hosts` is this
    // component's own prop (not a `Signal`, so it cannot be `Copy`-captured
    // the way the surrounding signals are), and the render body further
    // down needs its own, unmoved copy for the selector.
    let hosts_for_reseed = hosts.clone();
    use_effect(use_reactive(
        (&prefill.as_ref().map(|prefill| prefill.generation),),
        move |_| {
            // A new clone intent supersedes a still-waiting switcher pick.
            // The form may stay mounted, so clearing the parent alone cannot
            // revoke the child's mount-time copy.
            if prefill.is_some() {
                pending_initial_action.set(None);
            }
            let hosts = &hosts_for_reseed;
            let target = create_target();
            let previous = bound_target.peek().clone();

            // Applied BEFORE the host resolution below, and in the
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
                select_existing_directory(
                    &mut destination_draft,
                    &mut cwd,
                    &mut cwd_raw_seed,
                    &mut cwd_edited,
                    &mut folder_is_explicit,
                    &prefill.cwd,
                );
                folder_is_explicit.set(false);
                template_panel.set(None);
                let source_repo = prefill
                    .repo
                    .clone()
                    .filter(|_| prefill.replace_source.is_none());
                clone_checkout_naming.set(CloneCheckoutNaming {
                    source_repo: source_repo.clone(),
                    ..Default::default()
                });
                if let Some(repo) = source_repo {
                    // A second Clone of the same row still needs a new offer:
                    // equal repository/name inputs must not reuse its old preview.
                    destination_draft.set(DestinationDraft::github(repo));
                    preview_revision.with_mut(|revision| {
                        *revision = revision.checked_add(1).expect("preview revision exhausted")
                    });
                }
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
                // the dormant command field included, so switching a
                // structured clone to the command path starts from the
                // source's own command.
                reseed_cloned_field(
                    &mut invocation,
                    &mut invocation_raw_seed,
                    &mut invocation_edited,
                    &prefill.invocation,
                );
                // A command launch's clone carries its assertion, agent
                // type and resume command (SPEC.md, Clone). Anything else —
                // an agent launch, or a legacy session whose command is all
                // the launcher gets — leaves them for the user, the YOLO
                // assertion unanswered.
                match &prefill.command {
                    Some(command) => {
                        command_yolo.set(Some(command.yolo));
                        command_agent.set(command.agent);
                        command_resume_on.set(command.resume.is_some());
                        reseed_cloned_field(
                            &mut command_resume,
                            &mut command_resume_raw_seed,
                            &mut command_resume_edited,
                            command.resume.as_deref().unwrap_or_default(),
                        );
                        launch_tab.set(LaunchTab::Command);
                    }
                    None => {
                        command_yolo.set(None);
                        command_agent.set(None);
                        command_resume_on.set(false);
                        command_resume.set(String::new());
                        command_resume_raw_seed.set(None);
                        command_resume_edited.set(false);
                    }
                }
                if let Some(launch) = &prefill.launch {
                    // A structured snapshot is the source's explicit
                    // request, whereas its invocation is only the compiler's
                    // result. Preserve it for clone except for Pi's
                    // compatibility normalization: an older omitted
                    // permission is Pi's mandatory YOLO mode.
                    launch_tab.set(LaunchTab::Agent);
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
                    structured_permissions.set(crate::launch_composer::normalized_permissions(
                        launch.harness,
                        launch.permissions,
                    ));
                    structured_workspace_trust.set(
                        crate::launch_composer::normalized_workspace_trust(
                            launch.harness,
                            launch.workspace_trust,
                        ),
                    );
                    structured_workspace_trust_is_explicit.set(true);
                    // A restored structured snapshot is a real choice the
                    // source session made, not a passive default — it must
                    // filter recents exactly as it always has.
                    structured_permissions_is_explicit.set(true);
                } else {
                    launch_tab.set(LaunchTab::Command);
                    structured_harness.set(None);
                    structured_model_raw_seed.set(None);
                    structured_model_edited.set(false);
                    structured_model.set(None);
                    custom_model_harness.set(None);
                    structured_effort.set(None);
                    structured_permissions.set(None);
                    structured_workspace_trust.set(None);
                    structured_workspace_trust_is_explicit.set(false);
                    structured_permissions_is_explicit.set(true);
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
                // host decision from a clean slate, cleared BEFORE any
                // attempt to resolve it — otherwise a clone whose own host
                // cannot be confirmed (rejected below, or hostless) could
                // silently inherit whatever a PREVIOUS generation (or an
                // earlier manual pick) had left in these two signals.
                chosen_host.set(None);
                clone_host_state.set(match prefill.host {
                    None => CloneHostState::Unconfirmable, // F4: nothing to resolve safely
                    Some(_) => CloneHostState::Waiting,
                });
            }

            // Resolve (or re-resolve) THIS generation's own host binding.
            // Deliberately NOT gated on the generation transition
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
            }
            // An unconfirmable clone host leaves the selector at its ordinary
            // default. The launch seeded above is deliberately unaffected.

            mount_seeded.set(true);
            if previous != target {
                bound_target.set(target.clone());
                // Equality already compares the row and installation
                // fingerprint, so every target change here is a new intent.
                intent_key.set(None);
                // A refusal belongs to the host it came from. Left standing,
                // host A's refusal could be shown for host B.
                error.set(None);
            }
        },
    ));

    // Only the active creation surface determines the notice; the other
    // surface retains a draft that may describe a different harness. A typed
    // command is never classified as Cursor here.
    let cursor_launch =
        launch_tab() == LaunchTab::Agent && structured_harness() == Some(LaunchHarness::Cursor);
    let displayed_invocation = invocation.read().clone();

    let preview_agent_now = move || {
        format!(
            "{:?}:{:?}:{:?}:{:?}:{:?}:{}",
            launch_tab(),
            structured_harness(),
            structured_model(),
            structured_effort(),
            structured_permissions(),
            submitted_field(
                &invocation(),
                invocation_edited(),
                invocation_raw_seed.peek().as_deref()
            ),
        ) + &format!(
            ":{:?}:{:?}:{}:{}",
            command_yolo(),
            command_agent(),
            command_resume_on(),
            submitted_field(
                &command_resume(),
                command_resume_edited(),
                command_resume_raw_seed.peek().as_deref()
            )
        )
    };
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
    let mut naming = clone_checkout_naming.peek().clone();
    naming.retarget(destination_now.clone(), destination_draft().repo().cloned());
    if naming != *clone_checkout_naming.peek() {
        clone_checkout_naming.set(naming);
    }
    // Display, preview authority, retry binding, submit, draft snapshot and
    // request-key re-read must share this policy. A stray raw-title read
    // would make the create disagree with its preview and be refused as stale.
    let title_for_launch = move || {
        let destination = destination_draft();
        let naming = clone_checkout_naming();
        let seed = title_raw_seed();
        effective_title(
            &title(),
            title_edited(),
            seed.as_deref(),
            &destination,
            &naming,
        )
    };
    let mut proposed_authority = destination_draft().repo().cloned().and_then(|repo| {
        let host = hosts.iter().find(|host| Some(host.id) == selected)?;
        Some(PreviewAuthority {
            generation: 0,
            config_revision: observed_checkout_revision(),
            host: host.id.to_string(),
            incarnation: host.connection,
            installation_identity: host.identity.clone()?,
            repo,
            // Only built while a repository is the destination, hence `true`.
            title: Some(title_for_launch()),
            agent: preview_agent_now(),
        })
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
        let draft = destination_draft.peek().clone();
        if let DestinationDraft::Github { repo, .. } = draft {
            destination_draft.set(DestinationDraft::github(repo));
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
        // Not a bare `structured_permissions()` read: see
        // `structured_permissions_is_explicit`'s own doc for why the
        // passive memory seed must not act as a filter dimension.
        permissions: if structured_permissions_is_explicit() {
            structured_permissions()
        } else {
            None
        },
        workspace_trust: if structured_workspace_trust_is_explicit() {
            structured_workspace_trust()
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
    // A stored structured snapshot is provenance, rather than a promise that
    // a later release still supports every combination it named. Keep its
    // model and effort visible for clone, but refuse to turn an incompatible
    // known choice into a different launch by guessing a replacement. Pi's
    // omitted-permission compatibility rule was already applied while seeding.
    let structured_choice_error = structured_harness().and_then(|harness| {
        let selection = LaunchSelection {
            harness,
            model: structured_model(),
            effort: structured_effort(),
            permissions: structured_permissions(),
            workspace_trust: crate::launch_composer::normalized_workspace_trust(harness, structured_workspace_trust()),
        };
        (!crate::launch_composer::selection_fits_catalog(&selection, catalog_answer.as_deref())).then_some(
            "this saved choice is no longer supported by the current catalog; choose a compatible model or effort",
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
    } else {
        None
    };
    // The submit button's verb: "replace" for a replace-with prefill,
    // "launch" for everything else (an ordinary create OR a plain clone),
    // so the destructive half of what a click does is visible on the
    // button itself rather than only in a menu item clicked a moment
    // earlier. `launch-composer-launch-context` beside it is unchanged
    // either way — the host/folder summary is equally true of both verbs.
    let submit_verb = if is_replace_with { "replace" } else { "launch" };
    // What pressing "replace" stops, drawn from the same render the submit
    // handler below is created in: the warning under the button, and the
    // precondition that handler sends (see `replace_with_warning`).
    let (replace_warning, replace_guard) = if is_replace_with {
        replace_with_warning(replace_source_state.as_ref())
    } else {
        (None, crate::DeleteGuard::NothingAlive)
    };
    let current_launch = if launch_tab() == LaunchTab::Agent {
        structured_harness().map(|harness| {
            LaunchIntent::Structured(LaunchSelection {
                harness,
                model: structured_model(),
                effort: structured_effort(),
                permissions: crate::launch_composer::normalized_permissions(
                    harness,
                    structured_permissions(),
                ),
                workspace_trust: crate::launch_composer::normalized_workspace_trust(
                    harness,
                    structured_workspace_trust(),
                ),
            })
        })
    } else {
        command_intent(
            submitted_field(
                &invocation(),
                invocation_edited(),
                invocation_raw_seed.peek().as_deref(),
            ),
            command_yolo(),
            command_agent(),
            command_resume_on(),
            &submitted_field(
                &command_resume(),
                command_resume_edited(),
                command_resume_raw_seed.peek().as_deref(),
            ),
        )
        .map(LaunchIntent::Command)
    };
    let retry_binding = current_launch.and_then(|launch| {
        let draft = IntentBinding::of(
            selected,
            &hosts,
            cwd(),
            launch,
            title_for_launch(),
            prefill_for_submit
                .as_ref()
                .and_then(|prefill| prefill.replace_source.clone()),
        )?;
        let destination = destination_draft();
        let repo = destination.repo()?;
        let installation = hosts
            .iter()
            .find(|host| Some(host.id) == selected)?
            .identity
            .as_deref()?;
        github_attempt()
            .filter(|(original, _)| same_fresh_intent(original, &draft, repo, installation))
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
        let preview_state = match result {
            Ok(preview) if authority.accepts(&live, &preview) => PreviewState::Ready { authority: authority.clone(), preview },
            Ok(_) => PreviewState::Failed { authority: authority.clone(), message: "the preview is stale or belongs to a different host installation; select the repository again".into() },
            Err(failure) => {
                let mut naming = clone_checkout_naming.peek().clone();
                let default = naming.default_title(*title_edited.peek(), title_raw_seed.peek().as_deref(), destination_draft.peek().repo());
                if failure.occupied && default.as_ref() == authority.title.as_ref()
                    && default.is_some() && naming.advance(retry_binding.is_some()) {
                    clone_checkout_naming.set(naming);
                    return;
                }
                PreviewState::Failed { authority: authority.clone(), message: failure.message }
            },
        };
        if destination_draft.peek().repo() == Some(&authority.repo) {
            destination_draft.set(DestinationDraft::Github {
                repo: authority.repo,
                preview_state: Box::new(preview_state),
            });
        }
    }));
    let current_preview = match destination_draft() {
        DestinationDraft::Github { preview_state, .. } => match *preview_state {
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
    let fresh_destination_ready = matches!(destination_draft(), DestinationDraft::Existing { .. })
        || retry_binding.is_some()
        || current_preview.is_some();
    let command_requires_text = retry_binding.is_none();
    let launch_refusal = LaunchPrerequisites {
        busy,
        saving_template: template_panel().is_some() || template_saving(),
        host_available: selected_host_available,
        destination_ready: fresh_destination_ready,
        github_search: crate::launch_composer::scoped_query(&composer_search()).0
            == crate::launch_composer::SearchScope::Github,
        remembered_destination_valid,
        needs_harness: retry_binding.is_none()
            && launch_tab() == LaunchTab::Agent
            && structured_harness().is_none(),
        incompatible_choice: retry_binding.is_none()
            && launch_tab() == LaunchTab::Agent
            && structured_choice_error.is_some(),
    }
    .refusal();
    let displayed_preview = retry_binding
        .as_ref()
        .and_then(|binding| {
            binding
                .github_checkout
                .as_ref()
                .map(|checkout| checkout.preview.clone())
        })
        .or(current_preview);
    let summary_folder = match destination_draft() {
        DestinationDraft::Existing { cwd } => display_peer(&cwd),
        DestinationDraft::Github { repo, .. } => displayed_preview.as_ref().map_or_else(
            || format!("gh:{} (preview pending)", repo.identifier()),
            |preview| display_peer(&preview.cwd),
        ),
    };
    // The editable `cwd` seed belongs to existing-folder mode. A fresh
    // checkout gets its path only from an accepted preview (or the retained
    // retry binding), so the destination field must not echo that old seed.
    let (folder_field_value, folder_placeholder, checkout_mode) = match destination_draft() {
        DestinationDraft::Existing { .. } => (cwd(), "", false),
        DestinationDraft::Github { preview_state, .. } => (
            displayed_preview
                .as_ref()
                .map(|preview| display_peer(&preview.cwd))
                .unwrap_or_default(),
            if displayed_preview.is_some() {
                ""
            } else if matches!(*preview_state, PreviewState::Failed { .. }) {
                "checkout preview unavailable"
            } else {
                "waiting for checkout preview"
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
        title_raw_seed.peek().as_deref(),
        checkout_mode,
    );
    let clone_default = clone_checkout_naming().default_title(
        title_edited(),
        title_raw_seed.peek().as_deref(),
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
            .map(|preview| display_peer(&preview.basename))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let summary_model = structured_model()
        .map(|model| display_peer(&model))
        .unwrap_or_else(|| "default".to_string());
    let summary_effort = structured_effort()
        .map(crate::launch_composer::effort_value)
        .unwrap_or("default");
    let summary_trust = structured_workspace_trust()
        .map_or("default", |value| if value { "true" } else { "false" });
    let summary_permission = match structured_harness() {
        Some(harness) => {
            crate::launch_composer::normalized_permissions(harness, structured_permissions())
        }
        None => structured_permissions(),
    }
    .map(crate::launch_composer::permission_value)
    .unwrap_or("default");
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
            let mut selection = crate::launch_composer::select_recent(&entry);
            selection.permissions = crate::launch_composer::normalized_permissions(
                selection.harness,
                selection.permissions,
            );
            selection.workspace_trust = crate::launch_composer::normalized_workspace_trust(
                selection.harness,
                selection.workspace_trust,
            );
            structured_harness.set(Some(selection.harness));
            structured_model_raw_seed.set(selection.model.clone());
            structured_model_edited.set(false);
            structured_model.set(selection.model);
            custom_model_harness.set(crate::launch_composer::custom_model_owner(
                &entry.selection,
                &catalog,
            ));
            structured_effort.set(selection.effort);
            structured_permissions.set(selection.permissions);
            structured_workspace_trust.set(selection.workspace_trust);
            structured_workspace_trust_is_explicit.set(true);
            // Applying a recent is a deliberate whole-draft replacement, not
            // a passive default — its permissions choice must filter
            // further recents exactly as it always has.
            structured_permissions_is_explicit.set(true);
            invalidate_directory_browse(
                browse_generation,
                browse_request,
                browse_result,
                browse_error,
            );
            if let Some(repo) = entry.github_repo {
                remembered_destination.set(None);
                destination_draft.set(DestinationDraft::github(repo));
                preview_revision.with_mut(|value| {
                    *value = value.checked_add(1).expect("preview revision exhausted")
                });
            } else {
                select_existing_directory(
                    &mut destination_draft,
                    &mut cwd,
                    &mut cwd_raw_seed,
                    &mut cwd_edited,
                    &mut folder_is_explicit,
                    &entry.cwd,
                );
            }
            // Admission precedes the mode transition so a stale destination
            // cannot leave the user on a partially applied structured draft.
            // Once admitted, the row's meaning is the whole structured setup;
            // leaving command mode active would submit an unrelated dormant
            // invocation when Enter follows this callback.
            launch_tab.set(LaunchTab::Agent);
            composer_reset_reason.set(None);
            intent_key.set(None);
            true
        }
    });
    // Apply every catalog-row choice through one path so pointer and keyboard
    // activation cannot drift on reconciliation, error cleanup, or intent-key
    // invalidation. Custom drafts stay separate because they have no catalog
    // ownership to apply. Closing the list also discards the draft: the input
    // shows the selection once closed, and a draft that outlived the pick
    // would make the next Enter (the keystroke a person reaches for to
    // submit) apply the filter text as a custom id over the model just chosen.
    let apply_model_option = Callback::<crate::launch_composer::ModelOption>::new({
        let catalog = catalog_models.clone();
        let catalog_answer = catalog_answer.clone();
        move |option| {
            // Showing or hiding other harnesses' rows is a view toggle, not a
            // draft change: no busy gate, no intent-key invalidation.
            if let crate::launch_composer::ModelOption::ShowAll = option {
                model_show_all.set(!model_show_all());
                model_active.set(None);
                return;
            }
            if !draft_transition_allowed(ops) {
                return;
            }
            promote_fetched_history_snapshot(offered_history, create_target, fetched_history);
            match option {
                crate::launch_composer::ModelOption::HarnessDefault => {
                    structured_model.set(None);
                    structured_model_raw_seed.set(None);
                    structured_model_edited.set(false);
                    custom_model_harness.set(None);
                    composer_reset_reason.set(None);
                    model_draft_error.set(None);
                    model_draft.set(String::new());
                    model_open.set(false);
                }
                crate::launch_composer::ModelOption::Model { id, harness } => {
                    // A pick within the chosen harness is what the old model
                    // chip did: keep the harness, and clear an effort the
                    // picked model does not offer, saying so in terms of the
                    // MODEL. A pick that switches harness is the harness
                    // chip's reconciliation instead, whose message names the
                    // harness as well. The two messages differ on purpose:
                    // each tells the person which choice made the effort go.
                    if structured_harness() == Some(harness) {
                        let offered = catalog
                            .iter()
                            .find(|model| model.id == id && model.harness == harness)
                            .map(|model| model.efforts.clone())
                            .unwrap_or_default();
                        let chosen_effort = *structured_effort.peek();
                        match chosen_effort {
                            Some(effort) if !offered.contains(&effort) => {
                                structured_effort.set(None);
                                composer_reset_reason.set(Some(
                                    "the selected effort is not in Farhelm's offering for that model, so it was cleared".to_string(),
                                ));
                            }
                            _ => composer_reset_reason.set(None),
                        }
                        structured_model_raw_seed.set(None);
                        structured_model_edited.set(true);
                        structured_model.set(Some(id));
                        custom_model_harness.set(None);
                    } else {
                        let before = LaunchSelection {
                            harness: structured_harness().unwrap_or(harness),
                            model: structured_model(),
                            effort: structured_effort(),
                            permissions: structured_permissions(),
                            workspace_trust: structured_workspace_trust(),
                        };
                        let (selection, owner) =
                            crate::launch_composer::reconcile_harness_for_catalog_read(
                                LaunchSelection {
                                    model: Some(id),
                                    ..before.clone()
                                },
                                None,
                                harness,
                                catalog_answer.as_deref(),
                            );
                        composer_reset_reason.set(draft_reconciliation_reason(
                            &before,
                            &selection,
                            structured_permissions_is_explicit(),
                        ));
                        structured_harness.set(Some(selection.harness));
                        structured_model_raw_seed.set(None);
                        structured_model_edited.set(true);
                        structured_model.set(selection.model);
                        structured_effort.set(selection.effort);
                        structured_permissions.set(selection.permissions);
                        structured_workspace_trust.set(selection.workspace_trust);
                        custom_model_harness.set(owner);
                    }
                    model_draft_error.set(None);
                    model_draft.set(String::new());
                    model_open.set(false);
                }
                crate::launch_composer::ModelOption::ShowAll => unreachable!("handled above"),
            }
            model_active.set(None);
            intent_key.set(None);
        }
    });
    let active_search_harness = (*launch_tab.read() == LaunchTab::Agent)
        .then(&*structured_harness)
        .flatten();
    let active_search_model = (*launch_tab.read() == LaunchTab::Agent)
        .then(&*structured_model)
        .flatten();
    let search_text = composer_search();
    let (search_scope, repo_query) = crate::launch_composer::scoped_query(&search_text);
    let mut live_repository_authority = use_signal(|| None::<RepositoryAuthority>);
    let mut repository_generation = use_signal(|| 0_u64);
    let mut proposed_repository_authority = (search_scope
        == crate::launch_composer::SearchScope::Github)
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
    let repository_response = use_resource(move || {
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
                        "repository suggestions belong to a different host installation"
                            .to_string(),
                    )),
                    Err(error) => Some(Err(error)),
                }
            });
    let repository_note = match &repository_result {
        Some(Ok(reply)) => reply.scan_error.clone().or_else(|| {
            reply.truncated.then(|| {
                "repository search is incomplete; a valid owner/repo can still be selected".into()
            })
        }),
        Some(Err(message)) => Some(message.clone()),
        None => None,
    };
    // Search and the visible host selector must offer the same registry
    // snapshot; the helper owns no separate host cache or name resolution.
    let composer_hosts = hosts
        .iter()
        .map(|host| crate::launch_composer::ComposerHost {
            id: host.id,
            name: host.name.clone(),
            label: host.label(),
            local: host.local,
        })
        .collect::<Vec<_>>();
    let mut search_rows = crate::launch_composer::search_results(
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
    let templates_now = launch_templates.read().clone().unwrap_or_default();
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
    let search_result_groups = crate::launch_composer::grouped_search_results(search_rows);
    let catalog_models_for_search_input = catalog_models.clone();
    let composer_hosts_for_search_input = composer_hosts.clone();
    let template_names_for_search_input = template_names.clone();
    let template_targets = TemplateTargets {
        launch_tab,
        structured_harness,
        structured_model,
        structured_model_raw_seed,
        structured_model_edited,
        custom_model_harness,
        composer_reset_reason,
        structured_effort,
        structured_permissions,
        structured_permissions_is_explicit,
        structured_workspace_trust,
        structured_workspace_trust_is_explicit,
        invocation,
        invocation_raw_seed,
        invocation_edited,
        command_yolo,
        command_agent,
        command_resume_on,
        command_resume,
        command_resume_raw_seed,
        command_resume_edited,
        chosen_host,
        destination: destination_draft,
        cwd,
        cwd_raw_seed,
        cwd_edited,
        title,
        title_raw_seed,
        title_edited,
        clone_checkout_naming,
        template_error,
        intent_key,
    };
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
    let browse_cwd = cwd;
    let browse_cwd_raw_seed = cwd_raw_seed;
    let browse_cwd_edited = cwd_edited;
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
    let action_history_target = current_history_target.clone();
    let action_browse_base = browse_base.clone();
    let action_browse_hosts = browse_hosts.clone();
    let accept_search = use_callback(
        move |result: crate::launch_composer::ComposerSearchResult| {
            if !draft_transition_allowed(ops) {
                return false;
            }
            invalidate_directory_browse(
                browse_generation,
                browse_request,
                browse_result,
                browse_error,
            );
            promote_fetched_history_snapshot(offered_history, create_target, fetched_history);
            let mut browse_path = None;
            for edit in template_edits(
                result,
                &templates_now,
                &action_hosts,
                action_catalog.as_deref().unwrap_or_default(),
                is_replace_with,
                template_targets,
            ) {
                match edit {
                    TemplateEdit::Set(set) => set(),
                    TemplateEdit::Search(result) => {
                        browse_path = apply_composer_search_result(
                            result,
                            title,
                            title_edited,
                            chosen_host,
                            &action_hosts,
                            clone_host_state,
                            action_history_target.clone(),
                            live_destination,
                            remembered_destination,
                            history_activation_attempts,
                            destination_draft,
                            preview_revision,
                            cwd,
                            cwd_raw_seed,
                            cwd_edited,
                            folder_is_explicit,
                            launch_tab,
                            structured_harness,
                            structured_model,
                            structured_model_raw_seed,
                            structured_model_edited,
                            custom_model_harness,
                            structured_effort,
                            structured_permissions,
                            structured_permissions_is_explicit,
                            structured_workspace_trust,
                            structured_workspace_trust_is_explicit,
                            composer_reset_reason,
                            action_catalog.as_deref(),
                            intent_key,
                        );
                    }
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
                    cwd,
                    cwd_raw_seed,
                    cwd_edited,
                );
            }
            composer_search.set(String::new());
            composer_search_open.set(false);
            focus_composer_surface();
            true
        },
    );
    // Names need only mount seeding. Templates also need completed reads:
    // validating against temporary empty inputs would reject a valid choice.
    // While a template waits, the form presents a cancellable loading state
    // rather than accepting edits that the late application would overwrite.
    use_effect(use_reactive((&hosts_loaded,), move |(hosts_loaded,)| {
        let action = pending_initial_action.peek().clone();
        let Some(action) = action else {
            return;
        };
        if !mount_seeded() {
            return;
        }
        if matches!(
            action,
            crate::launch_composer::ComposerSearchResult::Template(_)
        ) && (!hosts_loaded
            || launch_templates.read().is_none()
            || launch_catalog.read().is_none())
        {
            return;
        }
        if accept_search.call(action) {
            pending_initial_action.set(None);
        }
    }));
    // Pointer and keyboard choices share their transition callbacks. Enter
    // runs these synchronously before form resubmission reads the live draft.
    let choose_harness = EventHandler::<LaunchHarness>::new({
        let catalog = catalog_for_harness.clone();
        move |harness| {
            if !draft_transition_allowed(ops) {
                return;
            }
            promote_fetched_history_snapshot(offered_history, create_target, fetched_history);
            let selection = LaunchSelection {
                harness: structured_harness().unwrap_or(harness),
                model: structured_model(),
                effort: structured_effort(),
                permissions: structured_permissions(),
                workspace_trust: structured_workspace_trust(),
            };
            let (selection, owner) = crate::launch_composer::reconcile_harness_for_catalog_read(
                selection,
                *custom_model_harness.peek(),
                harness,
                catalog.as_deref(),
            );
            composer_reset_reason.set(draft_reconciliation_reason(
                &LaunchSelection {
                    harness: structured_harness().unwrap_or(harness),
                    model: structured_model(),
                    effort: structured_effort(),
                    permissions: structured_permissions(),
                    workspace_trust: structured_workspace_trust(),
                },
                &selection,
                structured_permissions_is_explicit(),
            ));
            structured_harness.set(Some(selection.harness));
            structured_model_raw_seed.set(selection.model.clone());
            structured_model_edited.set(false);
            structured_model.set(selection.model);
            structured_effort.set(selection.effort);
            structured_permissions.set(selection.permissions);
            structured_workspace_trust.set(selection.workspace_trust);
            custom_model_harness.set(owner);
            launch_tab.set(LaunchTab::Agent);
            intent_key.set(None);
            focus_composer_surface();
        }
    });
    let choose_launch_tab = EventHandler::<LaunchTab>::new(move |tab| {
        if !draft_transition_allowed(ops) {
            return;
        }
        promote_fetched_history_snapshot(offered_history, create_target, fetched_history);
        launch_tab.set(tab);
        intent_key.set(None);
        focus_composer_surface();
    });
    let launch_from_choice = EventHandler::new(move |_| resubmit_composer());
    // The host selector and its reconciliation notes are built once for the
    // shared destination block. Command mode changes only launch controls, so
    // it has no second host path that could drift from this one.
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
            "data-tooltip": "host: the machine the session runs on",
            disabled: busy,
            onkeydown: move |event| enter_choice(event, ops.busy_now(), || {}, Some(launch_from_choice)),
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
    // The two resets share the recent-folder grid (see the folder links in the
    // rsx below) and carry their own class so the stylesheet can start them on
    // a row of their own: they are fixed destinations, not history, and the
    // row break is what says so now that no "·" separates the two kinds.
    let destination_resets = rsx! {
        button { r#type: "button", disabled: busy,
            class: "launch-composer-folder-reset",
            aria_label: "reset folder to home",
            "data-tooltip": "home: start in your home folder on this host",
            onclick: move |_| {
                if !draft_transition_allowed(ops) { return; }
                remembered_destination.set(None);
                promote_fetched_history_snapshot(
                    offered_history, create_target, fetched_history,
                );
                invalidate_directory_browse(
                    browse_generation, browse_request, browse_result, browse_error,
                );
                select_existing_directory(&mut destination_draft, &mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &mut folder_is_explicit, "~");
                intent_key.set(None);
            },
            "home"
        }
        button { r#type: "button", disabled: busy,
            class: "launch-composer-folder-reset",
            aria_label: "reset destination to local home",
            "data-tooltip": "local home: start in your home folder on this machine",
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
                select_existing_directory(&mut destination_draft, &mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &mut folder_is_explicit, "~");
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
                if !ops.busy_now() && !template_saving() {
                    on_cancel.call(());
                }
            },
        form {
            class: "create-session-form",
            // These are diagnostic state, not an authority channel. Keeping
            // the live connection and activation count on the mounted form
            // gives browser tests a synchronous observation point for races
            // that deliberately keep the visible result unchanged.
            "data-initial-templates-ready": launch_templates.read().is_some(),
            "data-history-fetched-revision": "{fetched_history_revision}",
            "data-composer-mode": if *launch_tab.read() == LaunchTab::Agent { "structured" } else { "command" },
            "data-browse-live-connection": "{live_browse_connection().unwrap_or_default()}",
            "data-browse-activation-attempts": "{browse_activation_attempts}",
            "data-browse-reply-completions": "{browse_reply_completions}",
            "data-history-activation-attempts": "{history_activation_attempts}",
            "data-remembered-destination-valid": "{remembered_destination_valid}",
            role: "dialog",
            // Browser validity bubbles cannot explain a refusal beside Launch.
            // Submit and the helm keep their ordinary validation authority.
            novalidate: true,
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
                // Native text-field and action-button submission has no choice
                // handler. Keep a held or composing Enter from launching there.
                if evt.key() == Key::Enter && (evt.is_auto_repeating() || evt.is_composing()) {
                    evt.prevent_default();
                }
                if evt.key() == Key::Escape && !ops.busy_now() && !template_saving() {
                    if template_panel().is_some() { template_panel.set(None); }
                    else { on_cancel.call(()); }
                }
            },
            onsubmit: move |evt| {
                evt.prevent_default();
                launch_attempted.set(true);
                error.set(None);
                if template_panel().is_some() || template_saving() { return; }
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
                let Some(op_guard) = ops.claim_guard() else {
                    return;
                };
                // The confirmation, if any, is matched against the key this
                // submit ends up using, below; the question itself is
                // answered either way. An answer already took it; a Launch
                // pressed while it is open supersedes it, which is
                // reconciliation rather than an answer, hence `clear`.
                yolo_refusal.clear();
                yolo_error.set(None);
                // Frozen HERE, from the live signals, and not touched again:
                // the minting await below can span further edits, and
                // re-reading across it would let the request's MODE differ
                // from the one the button was pressed on.
                let launch = if *launch_tab.peek() == LaunchTab::Agent {
                    let Some(harness) = *structured_harness.peek() else {
                        ops.release();
                        return;
                    };
                    let mut selection = LaunchSelection {
                        harness,
                        model: structured_model.peek().clone(),
                        effort: *structured_effort.peek(),
                        permissions: *structured_permissions.peek(),
                        workspace_trust: crate::launch_composer::normalized_workspace_trust(
                            harness, *structured_workspace_trust.peek(),
                        ),
                    };
                    selection.permissions = crate::launch_composer::normalized_permissions(
                        harness,
                        selection.permissions,
                    );
                    LaunchIntent::Structured(selection)
                } else {
                    // The RAW bytes while untouched, not the escaped display
                    // the field shows — see [`submitted_field`]
                    // (item2-review2.md F5).
                    // Replacing browser required-field validation keeps its
                    // empty-command refusal visible at Launch. Accepted fresh
                    // retries keep their existing exemption from this check.
                    if command_requires_text && invocation.peek().is_empty() {
                        error.set(Some("enter an agent command before launching".to_string()));
                        return;
                    }
                    let Some(command) = command_intent(
                        submitted_field(
                            &invocation.peek(),
                            *invocation_edited.peek(),
                            invocation_raw_seed.peek().as_deref(),
                        ),
                        *command_yolo.peek(),
                        *command_agent.peek(),
                        *command_resume_on.peek(),
                        &submitted_field(
                            &command_resume.peek(),
                            *command_resume_edited.peek(),
                            command_resume_raw_seed.peek().as_deref(),
                        ),
                    ) else {
                        error.set(Some(
                            "say whether this command runs without approval prompts (YOLO) before \
                             launching"
                                .to_string(),
                        ));
                        ops.release();
                        return;
                    };
                    LaunchIntent::Command(command)
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
                    ops.release();
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
                    .and_then(|prefill| prefill.replace_source.clone());
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
                    let unconfirmable = matches!(
                        *clone_host_state.peek(),
                        CloneHostState::Unconfirmable
                    );
                    error.set(Some(
                        if unconfirmable {
                            "replace with keeps the source session's own host, but that host \
                             could not be confirmed (the source reports a different installation \
                             now), so this create was not sent — replace with cannot proceed \
                             until the source's host is back; clone is the way to start a session \
                             elsewhere"
                        } else {
                            "replace with keeps the source session's own host, so this create was \
                             not sent — clone is the way to start a session on a different host"
                        }
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
                let Some(mut binding) = IntentBinding::of(
                    selected_now,
                    &hosts,
                    submitted_field(&cwd(), cwd_edited(), cwd_raw_seed.peek().as_deref()),
                    launch,
                    title_for_launch(),
                    replace_source,
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
                if crate::launch_composer::scoped_query(&composer_search()).0 == crate::launch_composer::SearchScope::Github {
                    return;
                }
                let fresh_draft_snapshot = move || (
                    destination_draft.peek().repo().cloned(),
                    *launch_tab.peek(), *structured_harness.peek(),
                    structured_model.peek().clone(), *structured_effort.peek(),
                    *structured_permissions.peek(),
                    submitted_field(&invocation.peek(), *invocation_edited.peek(), invocation_raw_seed.peek().as_deref()),
                    (*command_yolo.peek(), *command_agent.peek(), *command_resume_on.peek(), submitted_field(&command_resume.peek(), *command_resume_edited.peek(), command_resume_raw_seed.peek().as_deref())),
                    title_for_launch(),
                    *chosen_host.peek(),
                );
                let fresh_snapshot = fresh_draft_snapshot();
                let mut replaying_fresh = false;
                match destination_draft.peek().clone() {
                    DestinationDraft::Existing { cwd } => { binding.cwd = cwd; }
                    DestinationDraft::Github { repo, preview_state } => {
                        let installation = hosts.iter().find(|host| host.id == binding.host).and_then(|host| host.identity.as_deref());
                        let Some(installation) = installation else {
                            error.set(Some("the host installation is not verified; a checkout cannot be created".into()));
                            return;
                        };
                        let held = github_attempt.peek().clone();
                        if let Some((original, attempt)) = held.filter(|(original, _)| same_fresh_intent(original, &binding, &repo, installation)) {
                            binding = original;
                            intent_key.set(Some((attempt.key, binding.clone())));
                            replaying_fresh = true;
                        } else if let PreviewState::Ready { authority, preview } = *preview_state
                            && live_preview_authority.peek().as_ref().is_some_and(|live| authority.accepts(live, &preview))
                            && preview.installation_identity == installation
                            && preview.host == binding.host.to_string()
                            && Some(preview.incarnation) == connection_claim(&hosts, binding.host)
                            && authority.title.as_deref() == Some(binding.title.as_str())
                            && authority.agent == preview_agent_now()
                            && *preview_revision.peek() == *applied_preview_revision.peek()
                        {
                            binding.cwd = preview.cwd.clone();
                            binding.github_checkout = Some(GithubCheckoutRequest { repo: repo.identifier(), title: Some(binding.title.clone()), preview });
                        } else {
                            error.set(Some("wait for a current checkout preview before launching".into()));
                            return;
                        }
                    }
                }
                let fresh_authority = live_preview_authority.peek().clone();
                // An accepted request replays its recorded launch snapshot.
                // New requests still validate against today's catalog.
                if !replaying_fresh
                    && let LaunchIntent::Structured(selection) = &binding.agent
                    && !crate::launch_composer::selection_fits_catalog(selection, catalog_for_submit.as_deref())
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
                // The name the YOLO confirmation shows for this create's
                // host, from the same snapshot, should the helm ask.
                let yolo_host_name = hosts
                    .iter()
                    .find(|host| host.id == binding.host)
                    .map(HostOption::label)
                    .unwrap_or_else(|| "this host".to_string());
                // An admitted attempt is progress, not a refusal. Do not show
                // the busy prerequisite merely because its request is running.
                launch_attempted.set(false);
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
                        // A catalog refresh is not a new user choice, so it
                        // is frozen at the press. Structured controls are
                        // different. Every
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
                                workspace_trust: crate::launch_composer::normalized_workspace_trust(
                                    harness, *structured_workspace_trust.peek(),
                                ),
                            };
                            if *launch_tab.peek() != LaunchTab::Agent
                                || !crate::launch_composer::selection_fits_catalog(
                                    &selection,
                                    catalog_for_recheck.as_deref(),
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
                            cwd: if binding.github_checkout.is_some() { binding.cwd.clone() } else {
                                submitted_field(&cwd.peek(), *cwd_edited.peek(), cwd_raw_seed.peek().as_deref())
                            },
                            // Re-read through the preview's effective-name
                            // policy. The operation gate fixes the destination
                            // during key minting; the snapshot check catches drift.
                            title: title_for_launch(),
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
                    if bound.github_checkout.is_some()
                        && (fresh_draft_snapshot() != fresh_snapshot
                            || live_destination.peek().as_ref() != target_now.as_ref()
                            || (!replaying_fresh && *live_preview_authority.peek() != fresh_authority))
                    {
                        error.set(Some("the checkout draft changed while preparing the request; review the preview and press launch again".into()));
                        return;
                    }
                    // Only a confirmation given for THIS key counts; see
                    // `yolo_confirmed_key`.
                    let allow_yolo = yolo_confirmed_key.peek().as_deref() == Some(key.as_str());
                    // "Don't ask again": mark the host safe before anything
                    // is sent, and send nothing if that fails. The override
                    // is withdrawn on failure too, so a later plain Launch
                    // asks again rather than riding on this confirmation.
                    if let Some(answered) = stop_asking_for.filter(|answered| answered.refused_key == key)
                        && allow_yolo
                        && let Err(reason) = crate::yolo_confirm::stop_asking(&base, bound.host, &yolo_host_name).await
                    {
                        // The answer took the question down; it comes back,
                        // as a new opening, with the reason, so the user can
                        // pick another answer.
                        yolo_confirmed_key.set(None);
                        yolo_error.set(Some(reason));
                        open_yolo_question(answered.refused_key, answered.ask);
                        ops.release();
                        return;
                    }
                    let agent = match &bound.agent {
                        LaunchIntent::Command(command) => CreateAgent::Command(command),
                        LaunchIntent::Structured(selection) => CreateAgent::Structured(selection),
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
                    let create_result = if let Some(checkout) = &bound.github_checkout {
                        let attempt = github_attempt.peek().as_ref()
                            .filter(|(original, attempt)| original == &bound && attempt.key == key)
                            .map(|(_, attempt)| attempt.clone())
                            .unwrap_or_else(|| GithubAttempt::new(
                                key.clone(), api::fresh_create_body(agent, &key, bound.host, checkout),
                                checkout.preview.installation_identity.clone(),
                            ));
                        let mut attempt = attempt;
                        if allow_yolo {
                            api::confirm_yolo(&mut attempt.body);
                        }
                        // Publish before dispatch. A lost response must leave
                        // the exact payload available to the next explicit retry.
                        github_attempt.set(Some((bound.clone(), attempt.clone())));
                        // The body is the retained original; the source
                        // delete's precondition is this press's, matching
                        // the warning on screen now (see `submit_fresh_create`).
                        match api::submit_fresh_create(&base, bound.replace_source.as_deref(), replace_guard, &attempt.body).await {
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
                                Err(api::CreateRefusal { stale: false, yolo_confirmation: true, text })
                            }
                            Err(failure) => {
                                let retired = attempt.may_retire_after(&failure);
                                if retired {
                                    github_attempt.set(None);
                                    intent_key.set(None);
                                    preview_revision.with_mut(|revision| *revision = revision.checked_add(1).expect("preview revision exhausted"));
                                } else {
                                    github_attempt.set(Some((bound.clone(), attempt)));
                                }
                                Err(api::CreateRefusal::from(format!("{}; {}", failure.message(), if retired {
                                    "nothing was accepted; review the refreshed preview and press launch again"
                                } else {
                                    "the original request is retained; retry reconciles that request without allocating a different checkout"
                                })))
                            }
                        }
                    } else { match &bound.replace_source {
                        Some(source) => {
                            api::replace_session_with(
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
                            })
                        }
                        None => {
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
                    }};
                    match create_result {
                        Ok(session) => {
                            // Released before navigating: `on_created`
                            // unmounts this component, and a token released
                            // afterwards would be released by a task nobody
                            // is left to run.
                            ops.release();
                            let submitted_launch = match &bound.agent {
                                LaunchIntent::Structured(selection) => Some(selection.clone()),
                                LaunchIntent::Command(_) => None,
                            };
                            on_created.call(CreatedSession {
                                session: enrich_created_session(
                                    session,
                                    bound.host,
                                    created_host_identity,
                                ),
                                submitted_launch,
                            });
                        }
                        Err(e) => {
                            // Gated on the target this request was DISPATCHED
                            // for still being the one on screen: a refusal
                            // naming host A must not land under a form that
                            // has since been re-pointed at host B, where it
                            // would describe a machine the user is not looking
                            // at and may not even be true.
                            if create_target.peek().as_ref() == target_now.as_ref() {
                                let api::CreateRefusal { stale, yolo_confirmation, text: prose } = e;
                                if yolo_confirmation {
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
                                                    LaunchIntent::Structured(selection) => Some(selection),
                                                    LaunchIntent::Command(_) => None,
                                                },
                                            ),
                                        },
                                    );
                                    ops.release();
                                    return;
                                }
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
            if pending_initial_action().is_some() {
                p { role: "status", "loading session setup…" }
                button {
                    r#type: "button", class: "btn btn-neutral",
                    "data-tooltip": "cancel: discard this session setup",
                    onclick: move |_| on_cancel.call(()), "cancel"
                }
            } else {
            div { class: "launch-composer-topbar",
                span { class: "launch-composer-section-label", "new session" }
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
                    "data-tooltip": launch_refusal.unwrap_or(if is_replace_with { "replace: start this session, then delete the old one and its state" } else { "launch: start the session" }),
                    aria_describedby: replace_warning.as_ref().map(|_| REPLACE_WARNING_ID),
                    // Keep refused attempts reachable: native disabled would
                    // swallow both a click and implicit Enter before submit can
                    // say why. The operation guard still prevents another create.
                    aria_disabled: launch_refusal.is_some(),
                    "{submit_verb}"
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
                    class: "btn btn-neutral launch-composer-cancel",
                    "data-tooltip": "cancel: close without starting anything",
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
                    "cancel"
                }
                if *launch_tab.read() == LaunchTab::Agent {
                    button {
                        r#type: "button",
                        class: "btn btn-neutral launch-composer-reset",
                        "data-tooltip": "reset choices: clear the agent, model and effort, and go back to your remembered permissions and workspace trust",
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
                            // Reset returns the segment to the REMEMBERED
                            // value, not to "default" — it does not clear
                            // the memory itself, only re-applies it (SPEC.md's
                            // launch-composer carve-out). A fresh `peek` here
                            // rather than the value this dialog seeded from,
                            // since "reset" means "as if freshly opened now".
                            structured_permissions
                                .set(initial_structured_permissions(&preferences.0.peek()));
                            structured_workspace_trust
                                .set(structured_harness().map_or_else(
                                    || preferences.0.peek().remembered_workspace_trust,
                                    |harness| crate::launch_composer::normalized_workspace_trust(
                                        harness,
                                        preferences.0.peek().remembered_workspace_trust,
                                    ),
                                ));
                            // Back to a passive seed, exactly like a fresh
                            // open: reset does not turn the remembered value
                            // into a deliberate choice, so it must not start
                            // filtering recents either (see
                            // `structured_permissions_is_explicit`).
                            structured_permissions_is_explicit.set(false);
                            structured_workspace_trust_is_explicit.set(false);
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
                if !is_replace_with {
                    button { r#type: "button", class: "btn btn-neutral launch-composer-save-template",
                        "data-tooltip": "save as template: keep choices from this setup for another launch",
                        disabled: busy || template_saving() || template_panel().is_some(),
                        onclick: move |_| {
                            // Reopening an already mounted panel would replace
                            // its candidate while retaining its checkbox draft.
                            if !draft_transition_allowed(ops) || template_saving() || template_panel().is_some() { return; }
                            let state = launcher_snapshot(template_targets, &hosts_for_template_save, selected);
                            template_panel.set(Some((super::save_template::Candidate::from_state(&state,
                                super::save_template::ExplicitChoices {
                                    host: chosen_host().is_some() && clone_host_state() != CloneHostState::Bound,
                                    folder: folder_is_explicit(), name: title_edited(),
                                    permissions: structured_permissions_is_explicit(), trust: structured_workspace_trust_is_explicit(),
                                }), host_label_for_template_save.clone())));
                        }, "save as template"
                    }
                }
            }
            if let Some(err) = error.read().clone().or_else(|| {
                launch_attempted().then_some(launch_refusal).flatten().map(str::to_string)
            }) {
                // Refusals are visible beside the action they explain, even
                // when the rest of the dialog needs scrolling. Helm-owned words
                // still get the same escaping and directional isolation.
                PeerLine {
                    class: "create-session-error launch-composer-refusal".to_string(),
                    parts: vec![DetailPart::Peer(err)],
                }
            }
            if let Some((candidate, host_label)) = template_panel() {
                super::save_template::SaveTemplatePanel {
                    candidate, host_label, catalog: catalog_answer.clone(), saving: template_saving,
                    on_cancel: move |_| { template_panel.set(None); focus_composer_surface(); },
                    on_saved: on_template_saved,
                }
            }
            // "Replace with"'s warning sits directly under the replace button
            // it qualifies, for the same reason as the YOLO question below:
            // the button is the confirmation, and a warning out of view would
            // not be read before the press (SPEC.md "Replace with").
            // Polite live region: the warning can appear or change while the
            // launcher stays open, and the replace button names it as its
            // description, since pressing that button is the confirmation.
            if is_replace_with {
                div { id: REPLACE_WARNING_ID, class: "launch-composer-replace-warning", aria_live: "polite",
                    if let Some(warning) = &replace_warning {
                        "{warning}"
                    }
                }
            }
            // The YOLO question sits directly under Launch, where the click
            // that raised it happened. Rendered after every other section, it
            // landed below the launcher's visible edge and pressing Launch
            // seemed to do nothing until the user scrolled.
            //
            // Shown only while the draft still holds the refused request's
            // key: an edit that changes what would launch retires the key,
            // and with it a question that no longer describes the draft.
            if let Some(question) = yolo_refusal.current_key()
                && intent_key.read().as_ref().is_some_and(|(key, _)| *key == question.refused_key)
            {
                crate::yolo_confirm::YoloConfirmation {
                    ask: question.ask.clone(),
                    busy,
                    error: yolo_error(),
                    // Both answers resubmit the form themselves, and only
                    // once they have taken this opening of the question
                    // (`answer_launcher_yolo`). As native submit buttons they
                    // would submit it whether or not the question was still
                    // open, sending a cancelled draft again.
                    confirm_submits: false,
                    // The one-off also disarms any "don't ask again"
                    // request (`answer_launcher_yolo`): it must never mark
                    // the host.
                    on_confirm: {
                        let question = question.clone();
                        move |_| {
                            if answer_launcher_yolo(
                                &mut yolo_refusal,
                                &question,
                                false,
                                &mut yolo_confirmed_key.write(),
                                &mut yolo_stop_asking.write(),
                            ) {
                                resubmit_composer();
                            }
                        }
                    },
                    on_confirm_and_stop_asking: {
                        let question = question.clone();
                        move |_| {
                            if answer_launcher_yolo(
                                &mut yolo_refusal,
                                &question,
                                true,
                                &mut yolo_confirmed_key.write(),
                                &mut yolo_stop_asking.write(),
                            ) {
                                resubmit_composer();
                            }
                        }
                    },
                    on_cancel: move |_| {
                        // Only this opening: a cancel drawn for an earlier
                        // question leaves a later one (and its error) alone.
                        if yolo_refusal.take(&question).is_some() {
                            yolo_error.set(None);
                        }
                    },
                }
            }
            // Why the last template accepted from search was not applied;
            // nothing from it was (SPEC.md: all or nothing).
            if let Some(message) = template_error() {
                p { class: "launch-composer-template-error", role: "status", "{display_peer(&message)}" }
            }
            // Search belongs to the shared shell. A query can choose an
            // agent type, a model, a folder, or a saved setup; it never
            // manufactures a command from text alone, and it never chooses
            // the command tab itself.
            if *launch_tab.read() == LaunchTab::Agent {
                div { class: "launch-composer-summary", aria_live: "polite",
                    "model: "
                    span { class: "peer-value", dir: "ltr", "{summary_model}" }
                    " · effort: {summary_effort} · permissions: "
                    // Matched on the signal, not on display text, and spelled
                    // out per variant so every permission reads in the same
                    // lowercase register as "default": a Debug fallback would
                    // capitalize a future variant next to these.
                    if summary_permission == "yolo" {
                        span { class: "launch-composer-danger", "yolo" }
                    } else {
                        "{summary_permission}"
                    }
                    if structured_harness().is_some_and(LaunchHarness::offers_workspace_trust) {
                        " · trust: {summary_trust}"
                    }
                }
            }
            // Destination and its optional name are one draft regardless of
            // which launch controls happen to be active below.
            div {
                div {
                    class: "launch-composer-search",
                    onclick: move |evt| evt.stop_propagation(),
                    input {
                        r#type: "search",
                        role: "combobox",
                        aria_label: "search folders, harnesses, models, and efforts",
                        aria_expanded: composer_search_open(),
                        aria_controls: "launch-composer-search-results",
                        aria_activedescendant: (composer_search_open() && !search_results_for_keys.is_empty())
                            .then(|| format!("launch-composer-search-option-{}", composer_active_index)),
                        placeholder: "search names, hosts, folders, harnesses, models…",
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
                            // Preselect the exact-word match for THIS keystroke's
                            // results. Built from the history the promotion above
                            // just installed (read live from `offered_history`, not
                            // from a render-time clone that predates the promotion),
                            // so the index agrees with the list the next render
                            // draws from the same signal; the clamp at
                            // `composer_active_index` covers whatever still moves
                            // between this keystroke and a later render.
                            let promoted_history = offered_history
                                .peek()
                                .as_ref()
                                .and_then(|(target, history)| {
                                    // The SAME two-part predicate the render body's
                                    // `recent_history` applies: the offered target must
                                    // match both the parent-derived `create_target` and
                                    // the synchronously derived destination
                                    // (`live_destination`), because during the
                                    // one-render lag after a host change the former
                                    // still names the old host while the render will
                                    // already show nothing for it.
                                    (Some(target) == create_target().as_ref()
                                        && Some(target) == live_destination.peek().as_ref())
                                    .then(|| history.clone())
                                })
                                .unwrap_or_default();
                            let active_harness = (*launch_tab.peek()
                                == LaunchTab::Agent)
                                .then(&*structured_harness)
                                .flatten();
                            let active_model = (*launch_tab.peek()
                                == LaunchTab::Agent)
                                .then(&*structured_model)
                                .flatten();
                            let mut rows = crate::launch_composer::search_results(
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
                                        return;
                                    }
                                    if composer_search_open() && let Some(result) =
                                        search_results_for_keys.get(composer_active_index).cloned()
                                    {
                                        accept_search.call(result);
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
                                                    aria_selected: composer_active_index == index,
                                                    class: if matches!(result, crate::launch_composer::ComposerSearchResult::Recent(_)) {
                                                        if composer_active_index == index { "launch-composer-search-recent selected" } else { "launch-composer-search-recent" }
                                                    } else if composer_active_index == index { "selected" } else { "" },
                                                    "data-tooltip": search_result_tooltip(&result, &selected_host_label, *structured_harness.read()),
                                                    // Search recents use two visual spans as ordinary
                                                    // recents do. Their accessible label repeats the
                                                    // complete title with the result kind, instead of
                                                    // losing the separator where those spans meet.
                                                    aria_label: match &result {
                                                        crate::launch_composer::ComposerSearchResult::Recent(entry) => format!(
                                                            "Recent setup: {} · {} · {}",
                                                                display_peer(&crate::launch_composer::recent_destination_label(entry)), selected_host_label,
                                                            display_peer(&crate::launch_composer::selection_summary(&entry.selection)),
                                                        ),
                                                        _ => String::new(),
                                                    },
                                                    onclick: {
                                                        let result = result.clone();
                                                        move |_| {
                                                            accept_search.call(result.clone());
                                                        }
                                                    },
                                                    match &result {
                                                        crate::launch_composer::ComposerSearchResult::Name(name) => rsx! { "Set session name: {display_peer(name)}" },
                                                        crate::launch_composer::ComposerSearchResult::Host(host) => rsx! { "Host: {host.label}" },
                                                        // Folder actions alter only cwd; a recent
                                                        // setup visibly names every choice it owns.
                                                        crate::launch_composer::ComposerSearchResult::UsePath(folder)
                                                        | crate::launch_composer::ComposerSearchResult::Folder(folder) => rsx! { "Use this path: {display_peer(folder)}" },
                                                        crate::launch_composer::ComposerSearchResult::BrowsePath(folder) => rsx! { "Browse this path: {display_peer(folder)}" },
                                                        crate::launch_composer::ComposerSearchResult::Harness(harness) => rsx! { "Harness: {crate::launch_composer::harness_label(*harness)}" },
                                                        crate::launch_composer::ComposerSearchResult::Template(name) => rsx! { "Template: {display_peer(name)}" },
                                                        crate::launch_composer::ComposerSearchResult::Github(repo) => rsx! { "Fresh checkout: {repo.identifier()}" },
                                                        crate::launch_composer::ComposerSearchResult::Model { id, harness } => rsx! { "Model: {display_peer(id)} ({crate::launch_composer::harness_label(*harness)})" },
                                                        crate::launch_composer::ComposerSearchResult::Effort(effort) => rsx! { "Effort: {crate::launch_composer::effort_value(*effort)}" },
                                                        crate::launch_composer::ComposerSearchResult::Permissions(permission) => rsx! {
                                                            "Permissions: "
                                                            {match permission {
                                                                crate::launch_composer::ComposerPermission::Default => "default",
                                                                crate::launch_composer::ComposerPermission::Yolo => "yolo",
                                                            }}
                                                        },
                                                        crate::launch_composer::ComposerSearchResult::Trust(value) => rsx! { "Trust workspace: {value}" },
                                                        crate::launch_composer::ComposerSearchResult::Recent(entry) => rsx! {
                                                            span { class: "launch-composer-search-recent-destination", "Recent setup: {display_peer(&crate::launch_composer::recent_destination_label(&entry))} · {selected_host_label}" }
                                                            // Keep the dangerous permission as a semantic span
                                                            // instead of flattening it into the shared summary
                                                            // string, so search recents carry the same warning as
                                                            // the visible recent-setup rows.
                                                            span { class: "launch-composer-search-recent-selection",
                                                                "{display_peer(&crate::launch_composer::selection_summary_before_permissions(&entry.selection))} · permissions: "
                                                                if crate::launch_composer::selection_permission_value(&entry.selection) == "yolo" {
                                                                    span { class: "launch-composer-danger", "yolo" }
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
                if !recent_launches.is_empty() {
                    div { class: "launch-composer-recents",
                            span { class: "launch-composer-section-label", "recent setups" }
                            div { class: "launch-composer-recent-slots",
                            for (entry, summary, explicit, permission) in recent_launches.iter().take(3).map(|entry| (
                                entry,
                                crate::launch_composer::selection_summary(&entry.selection),
                                crate::launch_composer::selection_explicit_before_permissions(&entry.selection).join(" · "),
                                crate::launch_composer::selection_permission_value(&entry.selection),
                            )) {
                                button {
                                    r#type: "button",
                                    dir: "ltr",
                                    disabled: busy,
                                    "data-tooltip": "{display_peer(&crate::launch_composer::recent_destination_label(&entry))} · {selected_host_label} · {display_peer(&summary)}",
                                    // The visible row makes scanning cheaper, but
                                    // its tooltip and accessible name retain every
                                    // saved value that the one-line layout may
                                    // truncate. The punctuated string also keeps
                                    // adjacent spans from running together for
                                    // assistive technology.
                                    aria_label: "{display_peer(&crate::launch_composer::recent_destination_label(&entry))} · {selected_host_label} · {display_peer(&summary)}",
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
                                            resubmit_composer();
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
                                        span { class: "launch-composer-recent-host", "{selected_host_label}" }
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
                                        if explicit.is_empty() && permission == "default" {
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
                                        if permission != "default" {
                                            if !explicit.is_empty() { " · " }
                                            "permissions: "
                                            if permission == "yolo" {
                                                span { class: "launch-composer-danger", "yolo" }
                                            } else {
                                                "{permission}"
                                            }
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
            // The two columns preserve the form's destination-first tab
            // order in every mode. Only the launch-specific controls in the
            // choices column change with the launch-kind tab.
            if search_scope == crate::launch_composer::SearchScope::Github {
                if let Some(note) = repository_note {
                    div { class: "launch-composer-repository-note", role: "status", "{display_peer(&note)}" }
                }
            }
            if let DestinationDraft::Github { repo, preview_state } = destination_draft() {
                div { class: "launch-composer-checkout-preview", aria_live: "polite",
                    // Completion must be observable even while reconciliation
                    // hides a preview error behind the original accepted path.
                    "data-preview-state": match preview_state.as_ref() {
                        PreviewState::Pending => "pending",
                        PreviewState::Ready { .. } => "ready",
                        PreviewState::Failed { .. } => "failed",
                    },
                    "fresh checkout of {repo.identifier()} on {selected_host_label}"
                    if let Some(preview) = &displayed_preview {
                        div { dir: "ltr", "{display_peer(&preview.cwd)}" }
                    }
                    if retry_binding.is_some() {
                        div { "retry reconciles the original request at this path" }
                    } else {
                        match preview_state.as_ref() {
                            PreviewState::Pending => rsx! { div { "waiting for a current checkout preview" } },
                            PreviewState::Failed { message, .. } => rsx! { div { class: "create-session-error", "{display_peer(&message)}" } },
                            PreviewState::Ready { .. } => rsx! {},
                        }
                    }
                }
            }
            div { class: "launch-composer-columns",
                    div { class: "launch-composer-column-destination",
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
                                    // A proposed checkout path may not exist
                                    // yet, so Browse starts from the retained
                                    // existing folder rather than "this path"
                                    // while checkout mode is active.
                                    aria_label: if checkout_mode { "browse existing folders" } else { "browse this path" },
                                    "data-tooltip": if checkout_mode { "browse the existing folders on this host instead of a fresh checkout" } else { "browse: list the folders under this path on the host" },
                                    onclick: move |_| {
                                        if !draft_transition_allowed(ops) { return; }
                                        request_directory_browse(
                                            browse_base_for_folder.clone(), selected, &browse_hosts_for_folder, browse_target,
                                            submitted_field(&cwd(), cwd_edited(), cwd_raw_seed.peek().as_deref()),
                                            browse_generation, browse_request, browse_result, browse_error, browse_reply_completions,
                                            live_browse_connection, cwd, cwd_raw_seed, cwd_edited,
                                        );
                                    },
                                    // Center the whole label as one inline
                                    // unit when Browse moves below the host
                                    // selector at narrow widths.
                                    span {
                                        "browse folders on "
                                        span { class: "peer-value", dir: "ltr", "{selected_host_label}" }
                                    }
                                }
                            }
                            {host_notes.clone()}
                            input {
                                r#type: "text",
                                required: !checkout_mode,
                                readonly: checkout_mode,
                                autocomplete: "off",
                                autocorrect: "off",
                                autocapitalize: "none",
                                spellcheck: "false",
                                dir: "ltr",
                                value: "{folder_field_value}",
                                placeholder: "{folder_placeholder}",
                                disabled: busy,
                                aria_label: "folder",
                                oninput: move |evt| {
                                    if !draft_transition_allowed(ops) { return; }
                                    // A checkout path is evidence from the helm,
                                    // never an editable suggestion. The explicit
                                    // action below is the only way back to typing.
                                    if matches!(destination_draft(), DestinationDraft::Github { .. }) { return; }
                                    promote_fetched_history_snapshot(
                                        offered_history, create_target, fetched_history,
                                    );
                                    cwd.set(evt.value());
                                    destination_draft.set(DestinationDraft::Existing { cwd: evt.value() });
                                    cwd_edited.set(true);
                                    folder_is_explicit.set(true);
                                    remembered_destination.set(None);
                                    invalidate_directory_browse(
                                        browse_generation, browse_request, browse_result, browse_error,
                                    );
                                    intent_key.set(None);
                                },
                            }
                            if checkout_mode {
                                // The preview path may not exist yet. An
                                // existing-folder edit resumes from the prior
                                // editable seed, never from that proposal.
                                button {
                                    r#type: "button",
                                    class: "launch-composer-existing-folder",
                                    "data-tooltip": "use an existing folder instead of a fresh checkout",
                                    disabled: busy,
                                    onclick: move |_| {
                                        if !draft_transition_allowed(ops) { return; }
                                        remembered_destination.set(None);
                                        promote_fetched_history_snapshot(
                                            offered_history, create_target, fetched_history,
                                        );
                                        invalidate_directory_browse(
                                            browse_generation, browse_request, browse_result, browse_error,
                                        );
                                        let previous = submitted_field(
                                            &cwd(), cwd_edited(), cwd_raw_seed.peek().as_deref(),
                                        );
                                        select_existing_directory(
                                            &mut destination_draft, &mut cwd, &mut cwd_raw_seed,
                                            &mut cwd_edited, &mut folder_is_explicit, &previous,
                                        );
                                        intent_key.set(None);
                                    },
                                    "use existing folder"
                                }
                            }
                            // Recent folders are destination shortcuts, rendered as
                            // text links rather than chips so they read as history
                            // beneath the field they fill, not as a second picker.
                            // They sit in a grid, one path per cell. They used
                            // to be a wrapping run of inline links behind a
                            // "recent:" prefix with "·" between the two kinds,
                            // and paths of different lengths wrapped raggedly
                            // and left separators stranded at line ends. The
                            // group's accessible name still says what it holds.
                            div { class: "launch-composer-folder-links", aria_label: "recent folders",
                                for folder in recent_history.folders.iter().take(3) {
                                    button {
                                        r#type: "button",
                                        class: if submitted_field(&cwd(), cwd_edited(), cwd_raw_seed.peek().as_deref()) == folder.display_cwd { "selected" } else { "" },
                                        aria_pressed: submitted_field(&cwd(), cwd_edited(), cwd_raw_seed.peek().as_deref()) == folder.display_cwd,
                                        // A grid cell ellipsizes a long path at
                                        // its END, which is the part that tells
                                        // sibling checkouts apart, so the whole
                                        // path has to be reachable without
                                        // picking the link to find out.
                                        "data-tooltip": "{display_peer(&folder.display_cwd)}",
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
                                                select_existing_directory(&mut destination_draft, &mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &mut folder_is_explicit, &folder);
                                                intent_key.set(None);
                                            }
                                        },
                                        "{display_peer(&folder.display_cwd)}"
                                    }
                                }
                                {destination_resets.clone()}
                            }
                            // The optional name belongs to the shared destination
                            // block. Keeping one mounted input gives assistive
                            // technology one destination and preserves strict label
                            // lookup.
                            label { class: "launch-composer-name",
                                // The label stays the accessible wrapper; only its
                                // text takes the section-label caps, so the styling
                                // cannot inherit into what the person types.
                                span { class: "launch-composer-section-label", "name (optional)" }
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
                                    value: "{title_field_value}",
                                    placeholder: "{title_placeholder}",
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
                    }
                    div { class: "launch-composer-column-choices",
                        // One tab per launch kind. Switching only changes
                        // which draft is shown and submitted; each tab keeps
                        // its own (see `LaunchTab`).
                        // The tab pattern's keyboard half: one tab stop (the
                        // selected tab), and the arrow, Home and End keys
                        // select and focus the other tab, as screen readers
                        // announcing "tab, 1 of 2" lead people to expect.
                        div { class: "launch-kind-tabs", role: "tablist", "aria-label": "launch kind",
                            for (tab, label) in [(LaunchTab::Agent, "agent"), (LaunchTab::Command, "command")] {
                                button {
                                    r#type: "button",
                                    role: "tab",
                                    class: if launch_tab() == tab { "launch-kind-tab selected" } else { "launch-kind-tab" },
                                    "data-tooltip": if tab == LaunchTab::Agent { "agent: start a supported agent with its model, effort and permissions" } else { "command: run a command line you type, as written" },
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
                                        if !draft_transition_allowed(ops) {
                                            return;
                                        }
                                        promote_fetched_history_snapshot(
                                            offered_history, create_target, fetched_history,
                                        );
                                        launch_tab.set(target);
                                        intent_key.set(None);
                                        // After the render that moves the
                                        // tab stop, so focus lands on the tab
                                        // now selected.
                                        document::eval(
                                            "requestAnimationFrame(() => document.querySelector('.create-session-form .launch-kind-tab.selected')?.focus())",
                                        );
                                    },
                                    onclick: move |_| choose_launch_tab.call(tab),
                                    "{label}"
                                }
                            }
                        }
                        if launch_tab() == LaunchTab::Agent {
                        div { class: "launch-composer-choice launch-composer-harness-choice",
                            span { class: "launch-composer-section-label", "harness" }
                            div { class: "launch-composer-options",
                                for harness in crate::launch_composer::HARNESS_PICKER_ORDER {
                                    button {
                                        r#type: "button",
                                        class: if launch_tab() == LaunchTab::Agent && *structured_harness.read() == Some(harness) { "selected" } else { "" },
                                        aria_pressed: launch_tab() == LaunchTab::Agent && *structured_harness.read() == Some(harness),
                                        "data-tooltip": "use {crate::launch_composer::harness_label(harness)} for the session",
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
                                    span { role: "status", "model catalog unavailable: {message} " }
                                    button {
                                        r#type: "button",
                                        class: "btn btn-neutral",
                                        "data-tooltip": "retry: ask the host for its model list again",
                                        // Cleared first so the line goes away while the
                                        // read is out and comes back only if it fails
                                        // again; left in place, a retry would look like a
                                        // dead button until the new read settled.
                                        onclick: move |_| {
                                            launch_catalog.clear();
                                            launch_catalog.restart();
                                        },
                                        "retry"
                                    }
                                }
                            }
                            // These callbacks retain the create form's history and intent
                            // boundaries. The shared controls only report the user's edits.
                            LaunchControls {
                                harness: structured_harness(),
                                model: structured_model(),
                                model_raw_seed: structured_model_raw_seed(),
                                model_edited: structured_model_edited(),
                                effort: structured_effort(),
                                permissions: structured_permissions(),
                                workspace_trust: structured_workspace_trust(),
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
                                                promote_fetched_history_snapshot(
                                                    offered_history, create_target, fetched_history,
                                                );
                                                let before = LaunchSelection {
                                                    harness,
                                                    model: structured_model(),
                                                    effort: structured_effort(),
                                                    permissions: structured_permissions(),
                                                    workspace_trust: structured_workspace_trust(),
                                                };
                                                let selection = LaunchSelection {
                                                    harness,
                                                    model: Some(model),
                                                    effort: structured_effort(),
                                                    permissions: structured_permissions(),
                                                    workspace_trust: structured_workspace_trust(),
                                                };
                                                // An unknown catalog keeps the effort: see
                                                // `selection_fits_catalog`.
                                                if !crate::launch_composer::selection_fits_catalog(
                                                    &selection, catalog_answer.as_deref(),
                                                ) {
                                                    structured_effort.set(None);
                                                    composer_reset_reason.set(
                                                        crate::launch_composer::reconciliation_reset_reason(
                                                            &before,
                                                            &LaunchSelection {
                                                                effort: None,
                                                                ..selection.clone()
                                                            },
                                                        ),
                                                    );
                                                } else {
                                                    composer_reset_reason.set(None);
                                                }
                                                structured_model_raw_seed.set(None);
                                                structured_model_edited.set(true);
                                                structured_model.set(selection.model);
                                                custom_model_harness.set(Some(harness));
                                                model_draft_error.set(None);
                                                model_draft.set(String::new());
                                                model_open.set(false);
                                                model_active.set(None);
                                                intent_key.set(None);
                                            }
                                            crate::launch_composer::ModelEnterTarget::NeedsHarness(_) => {
                                                model_draft_error.set(Some(
                                                    "choose a harness before a custom model id".to_string(),
                                                ));
                                                intent_key.set(None);
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
                                    if !draft_transition_allowed(ops) { return; }
                                    promote_fetched_history_snapshot(
                                        offered_history, create_target, fetched_history,
                                    );
                                    structured_effort.set(effort);
                                    intent_key.set(None);
                                },
                                on_permissions: move |permission| {
                                    if !draft_transition_allowed(ops) { return; }
                                    promote_fetched_history_snapshot(
                                        offered_history, create_target, fetched_history,
                                    );
                                    structured_permissions.set(permission);
                                    structured_permissions_is_explicit.set(true);
                                    intent_key.set(None);
                                },
                                on_workspace_trust: move |choice| {
                                    if !draft_transition_allowed(ops) { return; }
                                    promote_fetched_history_snapshot(
                                        offered_history, create_target, fetched_history,
                                    );
                                    structured_workspace_trust.set(choice);
                                    structured_workspace_trust_is_explicit.set(true);
                                    intent_key.set(None);
                                },
                            }
                        }
                        if cursor_launch {
                            p { "Cursor session tracking and Resume are not supported." }
                        }
                        if *launch_tab.read() == LaunchTab::Command {
            // The raw command path: what the session runs, typed as one
            // command line.
            label {
                "agent command"
                input {
                    r#type: "text",
                    required: retry_binding.is_none(),
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    dir: "ltr",
                    value: "{displayed_invocation}",
                    disabled: busy,
                    oninput: move |evt| {
                        if !draft_transition_allowed(ops) {
                            return;
                        }
                        invocation.set(evt.value());
                        invocation_edited.set(true);
                        intent_key.set(None);
                    },
                }
            }
            // The YOLO assertion: a required choice with no default
            // (SPEC.md, Creation). Farhelm believes it and never reads the
            // command to check it.
            fieldset { class: "launch-command-yolo",
                legend { "runs without approval prompts" }
                label {
                    "data-tooltip": "yes: you assert this command acts without asking for approval; Farhelm marks it YOLO and does not check",
                    input {
                        r#type: "radio",
                        name: "launch-command-yolo",
                        onkeydown: move |event| enter_choice(event, ops.busy_now(), || {}, Some(launch_from_choice)),
                        checked: command_yolo() == Some(true),
                        disabled: busy,
                        onchange: move |_| {
                            if !draft_transition_allowed(ops) { return; }
                            command_yolo.set(Some(true));
                            intent_key.set(None);
                        },
                    }
                    "yes (YOLO)"
                }
                label {
                    "data-tooltip": "no: you assert this command asks before acting; Farhelm does not check",
                    input {
                        r#type: "radio",
                        name: "launch-command-yolo",
                        onkeydown: move |event| enter_choice(event, ops.busy_now(), || {}, Some(launch_from_choice)),
                        checked: command_yolo() == Some(false),
                        disabled: busy,
                        onchange: move |_| {
                            if !draft_transition_allowed(ops) { return; }
                            command_yolo.set(Some(false));
                            intent_key.set(None);
                        },
                    }
                    "no"
                }
            }
            label {
                "agent type"
                select {
                    class: "launch-command-agent",
                        onkeydown: move |event| enter_choice(event, ops.busy_now(), || {}, Some(launch_from_choice)),
                    "data-tooltip": "the agent this command runs, if any, so Farhelm can track and resume it",
                    disabled: busy,
                    onchange: move |evt| {
                        if !draft_transition_allowed(ops) { return; }
                        command_agent.set(
                            LaunchHarness::ALL
                                .iter()
                                .copied()
                                .find(|harness| harness_value(*harness) == evt.value()),
                        );
                        intent_key.set(None);
                    },
                    option { value: "", selected: command_agent().is_none(), "none" }
                    for harness in LaunchHarness::ALL.iter().copied() {
                        option {
                            value: harness_value(harness),
                            selected: command_agent() == Some(harness),
                            {crate::launch_composer::harness_label(harness)}
                        }
                    }
                }
            }
            if command_agent().is_some() {
                p { class: "launch-command-hint",
                    "Put {{farhelm_args}} in the command where Farhelm adds its own arguments."
                }
                label {
                    "data-tooltip": "resume: give the command a way to pick its conversation back up after a restart",
                    input {
                        r#type: "checkbox",
                        class: "launch-command-resume-toggle",
                        onkeydown: move |event| enter_choice(event, ops.busy_now(), || {}, Some(launch_from_choice)),
                        checked: command_resume_on(),
                        disabled: busy,
                        onchange: move |evt| {
                            if !draft_transition_allowed(ops) { return; }
                            command_resume_on.set(evt.checked());
                            intent_key.set(None);
                        },
                    }
                    "resume"
                }
                if command_resume_on() {
                    label {
                        "resume command"
                        input {
                            r#type: "text",
                            class: "launch-command-resume",
                            autocomplete: "off",
                            autocorrect: "off",
                            autocapitalize: "none",
                            spellcheck: "false",
                            dir: "ltr",
                            value: "{command_resume}",
                            disabled: busy,
                            oninput: move |evt| {
                                if !draft_transition_allowed(ops) { return; }
                                command_resume.set(evt.value());
                                command_resume_edited.set(true);
                                intent_key.set(None);
                            },
                        }
                    }
                }
            }
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
                        "data-tooltip": "use this folder: the session will start here",
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
                                select_existing_directory(&mut destination_draft, &mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &mut folder_is_explicit, &selected_cwd);
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
                            "data-tooltip": "use the parent folder instead",
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
                                select_existing_directory(&mut destination_draft, &mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &mut folder_is_explicit, &parent);
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
                            "data-tooltip": "use this folder: the session will start here",
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
                                select_existing_directory(&mut destination_draft, &mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &mut folder_is_explicit, &child);
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
            }
        }
        }
    }
}

#[cfg(test)]
mod tests {
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

    /// Spec: the command form describes no launch until the YOLO question
    /// is answered; once answered it carries the command, the answer, the
    /// declared agent type and, only while Resume is ticked AND a type is
    /// declared, the resume command.
    ///
    /// Why: SPEC.md makes the YOLO assertion a required choice with no
    /// default, so an unanswered form must not launch as "not YOLO". The
    /// Resume controls are shown only with a declared type, so a resume
    /// command left behind after the type is cleared must not be sent: the
    /// helm refuses it, naming a field the user can no longer see.
    #[test]
    fn the_command_form_needs_a_yolo_answer_and_drops_a_hidden_resume_command() {
        use farhelm_proto::LaunchHarness;
        assert_eq!(
            super::command_intent("c".into(), None, None, false, ""),
            None
        );
        assert_eq!(
            super::command_intent(
                "c {farhelm_args}".into(),
                Some(true),
                Some(LaunchHarness::Claude),
                true,
                "c --resume {conversation} {farhelm_args}",
            ),
            Some(super::CommandLaunch {
                command: "c {farhelm_args}".to_string(),
                yolo: true,
                agent: Some(LaunchHarness::Claude),
                resume: Some("c --resume {conversation} {farhelm_args}".to_string()),
            })
        );
        assert_eq!(
            super::command_intent(
                "c".into(),
                Some(false),
                Some(LaunchHarness::Claude),
                false,
                "c --resume {conversation} {farhelm_args}",
            )
            .and_then(|launch| launch.resume),
            None,
            "an unticked Resume sends no resume command"
        );
        assert_eq!(
            super::command_intent("c".into(), Some(false), None, true, "left over"),
            Some(test_command("c")),
            "clearing the agent type drops the resume command it hid"
        );
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
            list_sort: None,
            last_selected: None,
            compact: None,
            remembered_permissions: word.map(str::to_string),
            remembered_workspace_trust: None,
            skip_host_remove_confirmation: None,
            skip_host_setup_confirmation: None,
            feedback_contact: None,
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
        let mut original = IntentBinding::of(
            Some(1),
            &[option(1, "target", true)],
            "/old/bar-1".into(),
            LaunchIntent::Command(test_command("agent-a")),
            "work".into(),
            Some("source-a".into()),
        )
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
        current.incarnation = "reconnected".into();
        current.cwd = "/new/bar-2".into();
        current.github_checkout = None;
        assert!(same_fresh_intent(&original, &current, &repo, "install-a"));
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
        let hosts = vec![option(1, "this machine", true)];
        let command = || LaunchIntent::Command(test_command("agent"));
        let base = IntentBinding::of(
            Some(1),
            &hosts,
            "/tmp".to_string(),
            command(),
            "title".to_string(),
            None,
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
            None,
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
                agent: LaunchIntent::Command(test_command("other-agent")),
                ..base.clone()
            },
            IntentBinding {
                agent: LaunchIntent::Structured(LaunchSelection {
                    harness: LaunchHarness::Codex,
                    model: None,
                    effort: None,
                    permissions: None,
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
            IntentBinding::of(
                Some(1),
                &hosts,
                "/tmp".to_string(),
                command(),
                "title".to_string(),
                None,
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
        let nothing = || LaunchIntent::Command(test_command(""));
        assert!(
            IntentBinding::of(None, &hosts, String::new(), nothing(), String::new(), None)
                .is_none()
        );
        assert!(
            IntentBinding::of(
                Some(99),
                &hosts,
                String::new(),
                nothing(),
                String::new(),
                None
            )
            .is_none(),
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

    /// The raw invocation is carried on every prefill, a structured one
    /// included, which has no use for it until the user switches the mounted
    /// form to the command tab: leaving it unset there would let a stale,
    /// unrelated command surface then.
    #[farhelm_testtrace::test]
    fn prefill_from_carries_the_raw_invocation_even_for_a_structured_clone() {
        let session = Session {
            invocation: "claude --resume abc".to_string(),
            launch: Some(crate::SessionLaunch::Agent {
                start: Vec::new(),
                resume: None,
                selection: LaunchSelection {
                    harness: LaunchHarness::Claude,
                    model: None,
                    effort: None,
                    permissions: None,
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
            harness: LaunchHarness::Muse,
            model: Some("muse-spark-1.3-contributor".to_string()),
            effort: None,
            permissions: Some(LaunchPermission::Yolo),
            workspace_trust: None,
        };
        let session = Session {
            invocation: "muse --model muse-spark-1.3-contributor --yolo".to_string(),
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
            invocation: command.command.clone(),
            launch: Some(crate::SessionLaunch::Command(command.clone())),
            ..row_specimen("command")
        };
        let prefill = prefill_from(&session, 1);
        assert_eq!(prefill.command, Some(command));
        assert_eq!(prefill.launch, None);

        let legacy = Session {
            invocation: "claude --model opus".to_string(),
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
    /// counts as a new clone). `replace_source` is the one field
    /// `prefill_from` never sets: it builds a CLONE's prefill, and only
    /// `list::view::ListView`'s "replace with" handler turns that same
    /// value into a replace-with prefill afterward (see `prefill_from`'s
    /// own doc).
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
        assert_eq!(prefill.replace_source, None);
        assert_eq!(prefill.replace_source_opened, None);
    }

    /// Why this matters: Clone and Replace with copy the source's title, and
    /// a fresh checkout names its directory after the title and refuses a
    /// taken one, so the copied title (usually the source's own checkout)
    /// made every Clone or Replace with into `gh:` of the same repository
    /// fail with a directory conflict. Spec (SPEC.md, Fresh GitHub
    /// checkouts): with a fresh checkout as the destination, an unedited
    /// copied title is sent empty so the session is unnamed and gets
    /// `repo-N`; an edited title, a form with no copied seed, and every
    /// existing-folder launch keep `submitted_field`'s rule unchanged.
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
        let mut naming = CloneCheckoutNaming {
            source_repo: Some(repo),
            ..Default::default()
        };
        let raw = "source\tname";
        assert_eq!(
            effective_title("escaped", false, Some(raw), &source, &naming),
            "source\tname-clone"
        );
        naming.suffix = 2;
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

    /// Clone's default alone may search for a free checkout name. Switching
    /// hosts or repositories restarts that search; explicit edits, unnamed
    /// sources and Replace with must never acquire an automatic suffix.
    #[test]
    fn clone_checkout_names_are_defaults_scoped_to_the_destination() {
        let repo = GithubRepo::parse("acme/bar").unwrap();
        let other = GithubRepo::parse("acme/other").unwrap();
        let host = Some(CreateTarget::new(1, "install-a".into()));
        let mut naming = CloneCheckoutNaming {
            source_repo: Some(repo.clone()),
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
        assert_eq!(naming.suffix, 0);
        assert!(naming.advance(false));
        assert_eq!(
            naming
                .default_title(false, Some("bar-3"), Some(&repo))
                .as_deref(),
            Some("bar-3-clone-2")
        );
        naming.retarget(host.clone(), Some(repo.clone()));
        assert_eq!(naming.suffix, 2, "same destination keeps its search");
        naming.retarget(
            Some(CreateTarget::new(2, "install-b".into())),
            Some(repo.clone()),
        );
        assert_eq!(naming.suffix, 0, "new host searches from the base name");
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
        assert_eq!(naming.suffix, 0, "repository changes reset the counter");
        naming.source_repo = None;
        assert!(
            naming
                .default_title(false, Some("bar-3"), Some(&repo))
                .is_none()
        );
        for _ in 0..49 {
            assert!(naming.advance(false));
        }
        assert_eq!(naming.suffix, 50);
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
                .is_some_and(|text| text.starts_with("still running")),
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
        assert_eq!(prefill.replace_source.as_deref(), Some("s1"));
        assert_eq!(
            prefill.replace_source_opened,
            Some((SessionStatus::Exited { exit_code: Some(1) }, 1))
        );
        assert_eq!(
            CreatePrefill {
                replace_source: None,
                replace_source_opened: None,
                ..prefill
            },
            clone
        );
    }

    /// Checkout association chooses Clone's new repository, including a
    /// borrower below its root. Keep the source cwd too: Replace with and
    /// switching back to a folder need the original subdirectory unchanged.
    #[farhelm_testtrace::test]
    fn prefill_from_checkout_metadata_keeps_existing_subdirectory() {
        let mut session = Session {
            cwd: "/work/bar-1/subdir".to_string(),
            working_copy: Some(crate::github_checkout::WorkingCopyInfo {
                id: "checkout-1".to_string(),
                repo: GithubRepo::parse("acme/bar").unwrap(),
                canonical_path: "/work/bar-1".to_string(),
                origin_session_id: "origin".to_string(),
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

    /// item2-review2.md F5's untouched-vs-edited submission rule, exercised
    /// through this file's own field handling (a clone's directory,
    /// invocation and title) with a value a clone could plausibly carry: a
    /// right-to-left override inside an otherwise ordinary invocation.
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

    /// Why this matters: a launcher search result's hover text can quote a
    /// value the user typed or a peer supplied (a session name), and a
    /// tooltip is one attribute string that no DOM direction isolation
    /// reaches, so escaping is its only defense against an override
    /// character reordering what the user reads. Spec: a name result's
    /// tooltip carries the escaped name, never the raw control character,
    /// and the workspace-trust result says what false does for the selected
    /// agent (Codex runs the folder untrusted; Muse only adds no flag).
    #[farhelm_testtrace::test]
    fn search_result_tooltips_escape_names_and_follow_the_agent() {
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

        let codex = super::search_result_tooltip(
            &ComposerSearchResult::Trust(false),
            "local",
            Some(crate::LaunchHarness::Codex),
        );
        let muse = super::search_result_tooltip(
            &ComposerSearchResult::Trust(false),
            "local",
            Some(crate::LaunchHarness::Muse),
        );
        assert!(
            codex.contains("untrusted"),
            "Codex false runs the folder untrusted: {codex:?}"
        );
        assert!(
            muse.contains("no trust flag"),
            "Muse false only adds no flag: {muse:?}"
        );
    }

    /// Spec (`LauncherYoloQuestion`): an answer to the launcher's YOLO
    /// question records consent, and asks for a resubmit, only by taking the
    /// opening it was drawn for. Cancel followed by either answer in one
    /// event burst records no consent, no "don't ask again" request, and no
    /// resubmit; an answer drawn for an earlier opening takes nothing from a
    /// later one; a genuine answer records consent for the refused request's
    /// key, and only "don't ask again" also asks to mark the host.
    ///
    /// Why: the recorded consent is what lets the form's next submit of
    /// that request launch with no approval prompts (and, from Replace
    /// with, delete the source session). The launcher used to record it
    /// from the answer's own copy of the question, so an answer queued
    /// behind Cancel restored the consent the user had just withdrawn and
    /// submitted the draft. Consent kept under the key after a genuine
    /// answer is what lets a retry of the same request keep it.
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

        let question = |opening: u64| LauncherYoloQuestion {
            opening,
            refused_key: "intent-1".to_string(),
            ask: crate::yolo_confirm::YoloAsk {
                host: Some(7),
                host_name: "build box".to_string(),
                reason: crate::yolo_confirm::YoloReason::Asserted,
            },
        };

        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        let mut slot = SLOT.with(Cell::get).expect("the slot mounted");

        dom.in_runtime(|| {
            for stop_asking in [false, true] {
                let open = question(1);
                slot.open(open.clone(), ());
                // An armed request the stale answer must leave alone, so a
                // helper that wrote it regardless would show.
                let (mut confirmed, mut stop_for) = (None, Some(question(9)));
                assert!(slot.take(&open).is_some(), "premise: Cancel closes it");
                assert!(
                    !answer_launcher_yolo(
                        &mut slot,
                        &open,
                        stop_asking,
                        &mut confirmed,
                        &mut stop_for
                    ),
                    "an answer queued behind Cancel (stop asking: {stop_asking}) does not resubmit"
                );
                assert_eq!(confirmed, None, "and records no consent");
                assert!(
                    stop_for.is_some_and(|armed| armed.opening == 9),
                    "and changes no request to mark the host"
                );
            }

            // An answer drawn for an earlier opening.
            slot.open(question(2), ());
            let (mut confirmed, mut stop_for) = (None, None);
            assert!(!answer_launcher_yolo(
                &mut slot,
                &question(1),
                true,
                &mut confirmed,
                &mut stop_for
            ));
            assert_eq!(confirmed, None);
            assert!(
                stop_for.is_none(),
                "an answer for an earlier opening marks no host"
            );
            assert!(slot.is_open(), "the later question is left alone");

            // Positive controls. The one-off records consent and disarms a
            // stray "don't ask again" request.
            let mut stop_for = Some(question(2));
            assert!(answer_launcher_yolo(
                &mut slot,
                &question(2),
                false,
                &mut confirmed,
                &mut stop_for
            ));
            assert_eq!(confirmed.as_deref(), Some("intent-1"));
            assert!(stop_for.is_none(), "the one-off never marks the host");
            assert!(!slot.is_open());

            slot.open(question(3), ());
            let (mut confirmed, mut stop_for) = (None, None);
            assert!(answer_launcher_yolo(
                &mut slot,
                &question(3),
                true,
                &mut confirmed,
                &mut stop_for
            ));
            assert_eq!(confirmed.as_deref(), Some("intent-1"));
            assert!(stop_for.is_some_and(|answered| answered == question(3)));
        });
    }
}

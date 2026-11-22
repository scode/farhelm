//! Applying launch templates to the session launcher's draft.
//!
//! SPEC.md, "Launch templates": applying a template "makes the edits it contains, in the same way and with the same
//! effects as making them by hand", launch kind first, then agent type, then every other field, and a template that
//! does not apply is "refused with a message naming the field, and nothing from it is applied". SPEC_impl.md, "Launch
//! templates", says how: remove the assignments that repeat what the launcher's mode holds fixed, fold the templates
//! with the shared `farhelm_proto::launcher::apply_template` on a snapshot of the launcher (so a refusal changes
//! nothing), then replay the exact cleaned templates the fold accepted through the launcher's own edit methods. The
//! replay is what makes remembered wrapper prefixes, the folder re-seed on a host change and every other side effect
//! of a hand edit follow exactly as they would by hand, with no provenance tracking for templates.
//!
//! Here's the TLDR of the shape:
//!
//! - [`snapshot`] reads a [`LaunchDraft`] as the shared [`LauncherState`]. It is the one reader: Save as template
//!   captures the launcher through it too (SPEC_impl.md: the panel "shares the snapshot reader used for application").
//! - [`apply`] takes one batch (one `tl:` pick, one shortcut press, one completed sequence) and applies all of it or
//!   none of it. The fold decides; the replay only repeats what the fold accepted.
//! - The few things that have no hand edit are set directly: the session name's kind, a retired container choice's
//!   removal, and the initial prompt the fold leaves, including one kept dormant by a different launch type.
//!   A writer that put the folded state straight into the draft would be a second
//!   application path, and it would skip every side effect the hand edits carry.
//!
//! Ported from the old app at reference commit `24bc0580`: `launcher_snapshot`, `remove_matching_held_fields`,
//! `validated_template_batch_known` and `template_host` in `list/create_form.rs`, and `apply_template` with
//! `restart_template_refusal` in `list/create_form/draft.rs`. What the old app had around them for its browser (an
//! opening generation, modal snapshots, a queue that waited for template reads) is gone: templates come from the
//! store, and a batch is applied synchronously.
//!
//! A destination is a folder or a GitHub repository for a managed checkout: a template's repository is chosen through
//! [`LaunchDraft::choose_repository`], as picking it from the repository field's suggestions does, and the snapshot
//! reports the chosen repository as the destination (typed repository text that names no chosen repository is no
//! destination, as in the old app).
//!
//! An environment placement (a recipe and place, or a legacy recipe alone, which means that recipe on the Tensorlake
//! account) is checked against the store's recipe catalog ([`Context::known_recipes`]) and the registered hosts;
//! SPEC.md, "Creation": "Missing recipes or removed place hosts make that placement inapplicable". It is replayed
//! through the draft's Runs on edits (`LaunchDraft::choose_runs_on`, `choose_recipe` and `choose_container_host`), the
//! ones the launcher's Runs on switch and its Recipe and Container host selects make, folder `~` included. The
//! snapshot reports the draft's target and no host while one is set, as the old `launcher_snapshot` did, so the host
//! the Host row keeps underneath a new environment is never captured.

use std::collections::BTreeMap;

use farhelm_proto::launcher::{
    LaunchTemplate, LauncherKind, LauncherState, TemplateContext, TemplateDestination,
    TemplateFields, TemplateRefusal, apply_template,
};
use farhelm_proto::ui_channel::{HostStateView, HostView};
use farhelm_proto::{
    GithubRepo, LaunchHarness, LaunchPermission, Place, RepoError, parse_github_repo,
    split_permission_flags, split_wrapper_prefix,
};

use crate::launcher::draft::{
    Catalog, Escalation, Field, LaunchDraft, Mode, RunsOn, Tab, repo_identifier,
};
use crate::launcher::request::is_container_host;

/// What templates are applied against besides the draft itself, as the launcher reads it from the store when the
/// batch arrives.
pub struct Context<'a> {
    /// The hosts as the store holds them: a template's host field names an install identity, which is matched against
    /// these rows.
    pub hosts: &'a [HostView],
    /// The model catalog as the launcher read it; `None` while it has not answered, which checks only what each agent
    /// type itself declares (as an unread catalog does for a hand edit).
    pub catalog: Catalog<'a>,
    /// The wrapper prefix the helm remembers per agent type (`Preferences::remembered_wrapper_prefixes`), which a
    /// template's agent type loads exactly as picking that agent by hand does.
    pub remembered_wrappers: &'a BTreeMap<LaunchHarness, Vec<String>>,
    /// The folder New would seed for a host, which a template's host change re-seeds into a folder the user has not
    /// typed or picked ([`LaunchDraft::choose_host`]). The launcher owns that precedence (the open session's folder,
    /// the newest recent setup, `~`), so it is passed in rather than recomputed here.
    pub default_folder: &'a dyn Fn(i64) -> String,
    /// The names of the recipes the store's catalog lists, which a template's recipe must be one of. Empty while the
    /// catalog is unreadable, which refuses every placement as naming an unavailable recipe, as the old app's refusal
    /// path did once its catalog read failed.
    pub known_recipes: &'a [String],
}

/// The install identity a host row can be matched by, or `None` when it has no usable one.
///
/// A row in the identity-mismatch phase is excluded: there its recorded identity is the predecessor's while the row
/// reaches a different install, so matching it would aim a template at the successor, which SPEC.md rules out ("a
/// registry row retargeted to another install makes the template's host field inapplicable"). The old app's
/// `template_host` made the same exclusion, as the create default does.
pub fn install_identity(host: &HostView) -> Option<&str> {
    if matches!(host.state, HostStateView::IdentityMismatch { .. }) {
        return None;
    }
    host.identity.as_deref()
}

/// The launcher as the shared template rules read it: the active tab's kind, both tabs' fields, the host by its install
/// identity or, in its place, the new environment's target, the destination (the folder, or the chosen repository of a
/// managed checkout, or none while the repository field holds only typed text), and the session name with the kind a
/// template gave it.
///
/// The name is the one a template saves: a name a template set (a base or a fixed name) as it was written, and any
/// other name as the session would get it. So a template base is never the number a managed checkout moved it to
/// (SPEC.md, "Launch templates": "A template base is saved as its original text, never the numbered name displayed by
/// the launcher"), which Save as template relies on. This is the old app's `launcher_snapshot` rule.
///
/// Text that does not parse (custom flags or a wrapper prefix being typed) is left out rather than guessed at; the
/// launcher refuses to launch with it anyway. The initial prompt is the one the launch would send, so a dormant prompt
/// (kept on the Command tab or for an agent that cannot take one) is not part of the snapshot; [`apply`] puts it back
/// for the fold, which decides whether a template keeps it.
pub fn snapshot(draft: &LaunchDraft, hosts: &[HostView]) -> LauncherState {
    let agent = &draft.agent;
    let command = &draft.command;
    LauncherState {
        prompt: draft.active_prompt().map(str::to_owned),
        kind: Some(match draft.tab {
            Tab::Agent => LauncherKind::Agent,
            Tab::Command => LauncherKind::Command,
        }),
        harness: agent.harness,
        model: agent.model.clone(),
        model_owner: agent.model_owner,
        effort: agent.effort,
        permissions: agent.permissions,
        permission_flags: (agent.permissions == Some(LaunchPermission::Custom))
            .then(|| split_permission_flags(&agent.permission_flags).ok())
            .flatten(),
        wrapper_prefix: (!agent.wrapper.is_empty())
            .then(|| split_wrapper_prefix(&agent.wrapper).ok())
            .flatten(),
        container: agent.container.clone(),
        workspace_trust: None,
        command: command.command.clone(),
        yolo: command.yolo,
        command_agent: command.agent,
        // The old app's snapshot carried the resume text whenever Resume was switched on, declared agent or not; the
        // request leaves it out without one, but a held-value comparison and Save as template read the field itself.
        resume_command: command.resume_on.then(|| command.resume.clone()),
        // The Host row's host stays in the draft under a new environment, to give the row back when the user leaves
        // it; the launch does not go there, so it is no part of the snapshot (the old `launcher_snapshot`).
        host: draft
            .host
            .filter(|_| draft.target.is_none())
            .and_then(|id| hosts.iter().find(|host| host.id == id))
            .and_then(install_identity)
            .map(str::to_owned),
        // A placement is always the complete target; the legacy recipe-only field is only ever read from a template.
        recipe: None,
        target: draft.target.clone(),
        // While a checkout is chosen the folder is set aside, kept for choosing Folder again; it is not the
        // destination. An unchosen repository query is an editor draft, not a reusable destination: saved, it would
        // make the template fail whenever it is applied (the old `launcher_snapshot`).
        destination: match &draft.checkout {
            None => Some(TemplateDestination::Folder(draft.folder.value().clone())),
            Some(checkout) => checkout
                .repo
                .as_ref()
                .map(|repo| TemplateDestination::Github(repo_identifier(repo))),
        },
        // A name a template set, base or fixed, as held, never the number a checkout moved a base to; any other name
        // as the session would get it, which in a managed checkout drops an untouched copied Clone title.
        name: Some(match draft.name_kind {
            Some(_) => draft.name.value().clone(),
            None => draft.shown_name(),
        }),
        name_kind: draft.name_kind,
    }
}

/// What applying a batch did besides changing the draft, for the launcher's view to show beside the agent controls.
/// It follows the launcher's [`crate::launcher::draft::Edit`], with one difference the batch needs: a batch that made
/// no edit to the agent controls (one that only sets a folder or a name, say) leaves their notice alone rather than
/// clearing it.
#[derive(Debug, Default, PartialEq)]
pub struct Applied {
    /// The notice the agent controls show after the batch: `Some(text)` replaces the shown one and `Some(None)` clears
    /// it, as the last agent edit in the batch said; `None` when the batch made no edit that reports one, so the shown
    /// notice stays.
    pub notice: Option<Option<String>>,
    /// The last template with an applicable edit. Repeating only held values leaves the form's source alone.
    pub(crate) template: Option<String>,
    /// Some step of the batch changed the agent type, even if a later step changed it back. A banner an earlier action
    /// raised goes away then, since its Undo would also take back this batch.
    pub switched_agent: bool,
    /// The permission-escalation banner's content, if the batch raised one.
    pub escalation: Option<Escalation>,
}

/// Apply `templates` to `draft` in order, later ones winning where they overlap, all or nothing.
///
/// On success the draft holds every edit, made through its hand-edit methods, and [`Applied`] says what the view
/// shows for it. On a refusal the draft is untouched and the error is the sentence the launcher shows where it shows a
/// `tl:` refusal; the first refusal in template order wins.
///
/// The escalation banner is judged once per batch (SPEC_impl.md, "Launch templates"): the draft right after the batch's
/// last agent switch is compared with the draft from before the batch, and nothing is raised when the template that
/// made that switch chose the permission itself, since a permission a template sets is a direct choice and SPEC.md
/// says "Choosing a permission directly never triggers it" (the old app's `automatic_permission`). Edits after the last
/// switch do not count: a later template's permission is a direct choice too, not the switch's doing. The old app
/// judged each template of a press separately, from the draft before that template, so the two can differ when earlier
/// templates of one press changed the agent, the permission or the launch kind before the last switch; this banner's
/// Undo goes back to before the whole press.
pub fn apply(
    draft: &mut LaunchDraft,
    templates: &[LaunchTemplate],
    context: &Context<'_>,
) -> Result<Applied, String> {
    let cleaned = check(draft, templates, context)?;
    let before = draft.clone();
    let mut notice = None;
    // The draft right after the last template that changed the agent type, and whether that template chose the
    // permission itself.
    let mut last_switch: Option<(LaunchDraft, bool)> = None;
    for Cleaned {
        template,
        applied,
        repository,
    } in &cleaned
    {
        let harness = draft.agent.harness;
        replay(
            draft,
            &template.fields,
            applied,
            repository.as_ref(),
            context,
            &mut notice,
        );
        if draft.agent.harness != harness {
            last_switch = Some((draft.clone(), template.fields.permissions.is_some()));
        }
    }
    // Keep the fold's final prompt in the draft. The active launch decides whether it can send it.
    if let Some(last) = cleaned.last() {
        let prompt = last.applied.prompt.clone().unwrap_or_default();
        if draft.prompt != prompt {
            draft.edit_prompt(prompt);
        }
    }
    let switched_agent = last_switch.is_some();
    let escalation = last_switch.and_then(|(after_switch, chose_permission)| {
        (!chose_permission)
            .then(|| after_switch.escalation_since(&before))
            .flatten()
    });
    Ok(Applied {
        notice,
        template: cleaned
            .iter()
            .rev()
            .find(|entry| entry.template.fields != TemplateFields::default())
            .map(|entry| entry.template.name.clone()),
        switched_agent,
        escalation,
    })
}

/// One template as [`check`] accepted it, for the replay.
struct Cleaned {
    /// The template without the assignments the mode holds equal.
    template: LaunchTemplate,
    /// The fold's launcher state after it, which says what the draft cannot (which agent type a model goes to, whether
    /// it is a custom id, the name's kind).
    applied: LauncherState,
    /// Its GitHub repository destination, parsed once here so the replay chooses exactly what was checked.
    repository: Option<GithubRepo>,
}

/// The whole check, before anything changes: each template cleaned of the assignments the mode holds equal, then
/// folded in order, with what the replay needs of it ([`Cleaned`]).
fn check(
    draft: &LaunchDraft,
    templates: &[LaunchTemplate],
    context: &Context<'_>,
) -> Result<Vec<Cleaned>, String> {
    let held = snapshot(draft, context.hosts);
    let mut state = held.clone();
    // A partial template may name the agent type later in the same batch, so the fold starts with the prompt the
    // draft keeps even while it is dormant, and decides itself whether the batch keeps it.
    state.prompt = (!draft.prompt.is_empty()).then(|| draft.prompt.clone());
    let known_hosts: Vec<String> = context
        .hosts
        .iter()
        .filter(|host| !host.is_session_owned)
        .filter_map(install_identity)
        .map(str::to_owned)
        .collect();
    // The places the batch names that exist here: a place on a container host while the registry lists that host as
    // one (the launcher's own test, `is_container_host`), and a place on no host at all. Whether the host can take a
    // container right now (its Podman check) is the launcher's refusal, not this one: the old app's known places
    // ignored transient availability too. The old app also counted a host that cannot hold containers, and its launcher
    // then refused the launch; here the template is refused instead, since no launch could go there. The place's own
    // methods decide which kind it is, so no provider is named.
    let known_places: Vec<Place> = templates
        .iter()
        .filter_map(|template| template.fields.launch_target())
        .map(|target| target.place())
        .filter(|place| match place.container_host() {
            Some(id) => context
                .hosts
                .iter()
                .any(|host| host.id == id && is_container_host(host)),
            None => place.host().is_none(),
        })
        .collect();
    let fold_context = TemplateContext {
        catalog: context.catalog.unwrap_or(&[]),
        known_hosts: &known_hosts,
        known_recipes: context.known_recipes,
        known_places: &known_places,
        host_fixed: draft.holds(Field::Host),
    };
    let mut cleaned = Vec::with_capacity(templates.len());
    for template in templates {
        let mut template = template.clone();
        // A workspace-trust field from an older template is ignored when applied (SPEC.md, "Launch templates").
        template.fields.workspace_trust = None;
        remove_held(&mut template.fields, &held, draft);
        if let Some(refusal) = restart_refusal(draft, &template) {
            return Err(refusal.to_string());
        }
        state = apply_template(&state, &template, &fold_context)
            .map_err(|refusal| refusal.to_string())?;
        // A repository the shared fold accepts as text must still be one the launcher can choose; checked here, before
        // the next template, so the first refusal in the batch is the one reported, in the old app's words.
        let repository = match &template.fields.destination {
            Some(TemplateDestination::Github(repo)) => {
                Some(parse_github_repo(repo).map_err(|error| {
                    TemplateRefusal {
                        template: template.name.clone(),
                        field: "destination",
                        reason: format!(
                            "{repo:?} is not a GitHub repository ({})",
                            repository_refusal(&error)
                        ),
                    }
                    .to_string()
                })?)
            }
            Some(TemplateDestination::Folder(_)) | None => None,
        };
        cleaned.push(Cleaned {
            template,
            applied: state.clone(),
            repository,
        });
    }
    Ok(cleaned)
}

/// Why `owner/repo` text is not a repository, in the old app's two sentences (its `GithubRepo::parse` at reference
/// commit `24bc0580`): text without a pair at all, or a pair the rules refuse (a URL, a branch suffix, a bad name).
fn repository_refusal(error: &RepoError) -> &'static str {
    match error {
        RepoError::Empty | RepoError::MissingSeparator => "Enter a GitHub repository as owner/repo",
        RepoError::ExtraSeparator
        | RepoError::InvalidOwner
        | RepoError::InvalidRepo
        | RepoError::TooLong => {
            "Enter a valid GitHub owner/repo pair without a URL or branch suffix"
        }
    }
}

/// Remove the assignments that repeat a value the mode holds fixed, so they apply as nothing (SPEC.md, "Restart
/// with": "Templates and template shortcuts may repeat a held value as a no-op"; "A template containing only equal
/// held values applies silently as nothing").
///
/// They must be gone before the fold and before the replay alike: choosing even the same agent type again reconciles
/// the agent choices (which can clear a model, an effort or a historical unset permission), and choosing the host again
/// would re-seed placement. Equality follows SPEC_impl.md: the host by its usable recorded install identity, the
/// folder by its text ignoring trailing slashes (never expanded or resolved, since the GUI cannot resolve a remote
/// path), the name by its raw bytes, removed together with its kind. A held assignment that differs is left for
/// [`restart_refusal`] and the fold to refuse.
fn remove_held(fields: &mut TemplateFields, held: &LauncherState, draft: &LaunchDraft) {
    let restart = matches!(draft.mode, Mode::RestartWith(_));
    if (restart || draft.holds(Field::Host)) && fields.host.is_some() && fields.host == held.host {
        fields.host = None;
    }
    if !restart {
        return;
    }
    if fields.kind.is_some() && fields.kind == held.kind {
        fields.kind = None;
    }
    let held_agent = match held.kind {
        Some(LauncherKind::Command) => held.command_agent,
        _ => held.harness,
    };
    if fields.agent.is_some() && fields.agent == held_agent {
        fields.agent = None;
    }
    if fields
        .container
        .as_ref()
        .is_some_and(|container| *container == held.container)
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
        // A kind without its name does not apply, and a repeated name must not turn the session's raw title into a
        // template base.
        fields.name_kind = None;
    }
}

/// Preview only a mode's held-field refusal. Catalog and placement checks still run when the template is applied.
/// Use the same no-op rule as the actual apply so a template can repeat the current host, folder or agent.
pub(super) fn held_refusal(
    draft: &LaunchDraft,
    template: &LaunchTemplate,
    held: &LauncherState,
) -> Option<TemplateRefusal> {
    let mut template = template.clone();
    remove_held(&mut template.fields, held, draft);
    restart_refusal(draft, &template).or_else(|| {
        let fields = &template.fields;
        (draft.holds(Field::Host)
            && (fields.host.is_some() || fields.recipe.is_some() || fields.target.is_some()))
        .then_some(TemplateRefusal {
            template: template.name,
            field: "host",
            reason: "cannot change here: this dialog keeps the session's host".into(),
        })
    })
}

/// Restart with's refusal of a template that still sets a field the mode holds after [`remove_held`], naming the
/// first such field in the old app's order. Clearing a command's resume choice is refused there too (SPEC.md,
/// "Restart with": "Initial prompts and clearing a command's resume choice remain refused"). An environment placement
/// is not named here, as the old app's `restart_template_refusal` did not name it: the shared fold refuses it under
/// "host", since the mode holds the host (`TemplateContext::host_fixed`).
fn restart_refusal(draft: &LaunchDraft, template: &LaunchTemplate) -> Option<TemplateRefusal> {
    if !matches!(draft.mode, Mode::RestartWith(_)) {
        return None;
    }
    let fields = &template.fields;
    let field = if fields.kind.is_some() {
        "launch kind"
    } else if fields.agent.is_some() {
        "agent type"
    } else if fields.host.is_some() {
        "host"
    } else if fields.destination.is_some() {
        "destination"
    } else if fields.name.is_some() {
        "session name"
    } else if fields.prompt.is_some() {
        "initial prompt"
    } else if fields.container.is_some() {
        "container settings"
    } else if draft.tab == Tab::Command && fields.resume_command == Some(None) {
        "resume choice"
    } else {
        return None;
    };
    Some(TemplateRefusal {
        template: template.name.clone(),
        field,
        reason: format!("cannot change here: this dialog keeps {field} fixed for restart"),
    })
}

/// Make one cleaned template's edits through the draft's hand-edit methods, in SPEC.md's order. `applied` is the
/// fold's state after this template, which says what the draft alone cannot: which agent type a model belongs to,
/// whether it is a custom id, and the name's kind. `repository` is the template's repository destination as `check`
/// parsed it. `notice` is [`Applied::notice`] so far: each edit that reports a notice replaces it, and choosing the
/// default model clears it, as the launcher's view does for the same hand edits.
fn replay(
    draft: &mut LaunchDraft,
    fields: &TemplateFields,
    applied: &LauncherState,
    repository: Option<&GithubRepo>,
    context: &Context<'_>,
    notice: &mut Option<Option<String>>,
) {
    let remembered =
        |harness: LaunchHarness| context.remembered_wrappers.get(&harness).map(Vec::as_slice);

    // 1. The launch kind, before anything else: the same agent field means an agent launch's choice or a command's
    // declared agent depending on it.
    if let Some(kind) = fields.kind {
        draft.choose_tab(match kind {
            LauncherKind::Agent => Tab::Agent,
            LauncherKind::Command => Tab::Command,
        });
    }
    let kind = applied.kind.unwrap_or(LauncherKind::Agent);

    // 2. The agent type.
    if let Some(agent) = fields.agent {
        match kind {
            LauncherKind::Agent => {
                *notice = Some(
                    draft
                        .choose_harness(agent, context.catalog, remembered(agent))
                        .notice,
                );
            }
            LauncherKind::Command => draft.choose_command_agent(Some(agent)),
        }
    }

    // 3. The agent launch's choices. The fold has refused any of these on the command kind.
    match &fields.model {
        Some(Some(id)) => {
            // A known model can pick its agent even when the template named only the model. A custom ID already
            // has an agent from the template or draft, so it takes the same edit as typing it in that agent's field.
            if let Some(harness) = applied.harness {
                let spelled = harness.catalog_model_id(id);
                let listed = context.catalog.is_some_and(|catalog| {
                    catalog
                        .iter()
                        .any(|model| model.harness == harness && model.id == spelled)
                });
                let edit = if listed {
                    draft.choose_catalog_model(harness, id, context.catalog, remembered(harness))
                } else {
                    draft.choose_custom_model(id, context.catalog)
                };
                *notice = Some(edit.notice);
            }
        }
        Some(None) => {
            draft.choose_default_model();
            *notice = Some(None);
        }
        None => {}
    }
    if let Some(effort) = fields.effort {
        draft.choose_effort(effort);
    }
    if let Some(permissions) = fields.permissions {
        // A template's explicit reset asks for today's default. In Restart with, whose draft shows the permission the
        // stored command was built with, that must not restore a historical unset permission and look unchanged when
        // the helm would compile the guarded default (the old app's `apply_template_permissions`).
        let permissions = match permissions {
            None if matches!(draft.mode, Mode::RestartWith(_)) => draft
                .agent
                .harness
                .and_then(|harness| harness.request_permission(None)),
            chosen => chosen,
        };
        draft.choose_permissions(permissions);
        if permissions == Some(LaunchPermission::Custom) {
            let flags = fields.permission_flags.as_deref().unwrap_or_default();
            draft.edit_permission_flags(farhelm_proto::join_permission_flags(flags));
        }
    }
    if let Some(prefix) = &fields.wrapper_prefix {
        // A wrapper's words keep the quotes and escapes they were typed with, so its editor text is them joined; an
        // explicit null empties the editor, overriding a remembered prefix.
        draft.edit_wrapper(
            prefix
                .as_ref()
                .map(|words| words.join(" "))
                .unwrap_or_default(),
        );
    }
    if fields.container.is_some() {
        // The fold refuses a stored container object, so what is left here is the explicit null that removes one.
        draft.clear_retired_container();
    }

    // 4. The command launch's fields.
    if let Some(command) = &fields.command {
        draft.edit_command(command.clone());
    }
    if let Some(yolo) = fields.yolo {
        draft.choose_command_yolo(yolo);
    }
    if let Some(resume) = &fields.resume_command {
        // Present text switches Resume on with it; an explicit null switches it off and empties the text, as the old
        // app's `apply_template_resume` did.
        draft.choose_command_resume(resume.is_some());
        draft.edit_command_resume(resume.clone().unwrap_or_default());
    }

    // 5. The destination and the session name.
    if let Some(identity) = &fields.host
        && let Some(host) = context.hosts.iter().find(|host| {
            !host.is_session_owned && install_identity(host) == Some(identity.as_str())
        })
    {
        draft.choose_host(host.id, (context.default_folder)(host.id));
    }
    // The placement after the host, as the fold takes them; the fold refuses both in one template, so at most one of
    // the two applies here, and a later template's host leaves this placement (`choose_host` clears the target). A
    // new environment is entered as the Runs on switch enters one, which also puts the folder at its home; on the
    // segment already chosen the switch changes nothing, so the recipe and container host are then chosen as their own
    // fields would be (on a newly entered segment they already hold these values). A provider's place is fixed by its
    // container host, or it has one place only (the Tensorlake account); a provider with several places that are not
    // on a host needs its own edit here.
    if let Some(target) = fields.launch_target() {
        draft.choose_runs_on(RunsOn::of(Some(&target)), Some(target.clone()));
        draft.choose_recipe(target.recipe().to_owned());
        if let Some(host) = target.place().container_host() {
            draft.choose_container_host(host);
        }
    }
    // A repository is chosen as the repository field's suggestion is, so the launcher's own effects follow (a new
    // checkout preview among them).
    if let Some(repo) = repository {
        draft.choose_repository(repo.clone());
    } else if let Some(TemplateDestination::Folder(folder)) = &fields.destination {
        draft.edit_folder(folder.clone());
    }
    if let Some(name) = &fields.name {
        draft.edit_name(name.clone());
        // The fold always records a kind with a name it applied, base when the template named none.
        if let Some(kind) = applied.name_kind {
            draft.set_template_name_kind(kind);
        }
    }
}

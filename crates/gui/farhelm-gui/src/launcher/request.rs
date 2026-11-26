//! From a draft to a launch: whether it can launch, why not, and the request body it sends.
//!
//! SPEC.md, "Creation": "an incomplete attempt says why beside Launch, as does a refused Launch press, and the
//! greyed-out button's hover names what is missing". [`LaunchDraft::launch_refusal`] is that one reason, the first that
//! applies in the old app's order (`LaunchPrerequisites::refusal` and the submit-time checks in `create_form.rs` at
//! reference commit `24bc0580`), worded as it was. The launcher shows it as Launch's hover text while the draft cannot
//! launch, and beside Launch after a press. The helm validates the request again; what is checked here is what the
//! user can fix before sending.
//!
//! Where a shared rule's own error is more precise than the reason (a wrapper prefix or custom flags that do not
//! parse), the launcher's view shows that error beside the field itself, as the old app did; the reason here only says
//! which field to fix. Two departures from the old order, both on purpose: a command the shared command-launch rule
//! refuses is refused here for New, Clone and Replace with too (the old app left it to the helm, which refuses the same
//! way), and Restart with's "launch settings changed" check is a reason like the others rather than a check made only
//! at the press. A resumable host can accept the press. Its actual request body still needs the fresh connection
//! that the launcher's wake obtains (SPEC.md, "Topology").
//!
//! The bodies are `farhelm_proto::session_requests`' types, built only from a draft that passes
//! [`LaunchDraft::launch_refusal`]. What the old app's bodies carried for its retry keys is gone: the GUI sends none
//! (SPEC.md, "Creation").
//!
//! A launch into a new environment (New container, New sandbox) has no host yet: in place of the host's reasons it is
//! checked against the pushed recipe catalog, the Tensorlake account the launcher read (a sandbox's API key and SSH
//! key) and the container host's pushed Podman check, in the old launcher's words and, among themselves, its order
//! (`sandbox_reason` in `create_form.rs` and `recipe_refusal` in `sandbox_sessions.rs`). Together they take the host's
//! place, first, where the old launcher checked them after the agent tab's reasons: a deliberate departure, so a draft
//! that cannot name its environment says so before anything else. A managed checkout there needs only a chosen
//! repository and no named conversation: the environment has no installation to verify and no folder to preview yet,
//! and the helm takes the repository and naming intent until the new Farhelm connects (SPEC.md, "Managed checkouts": "A
//! recipe-target environment launch is another exception"). It sends its own body to its own route
//! ([`LaunchDraft::environment_body`]), never the ordinary create, which an older helm would run on its own machine.
//!
//! A managed checkout launches only with a current preview of its folder in hand (SPEC.md, "Managed checkouts":
//! "Before Launch becomes available, the composer shows the exact host and path"), which the view reads from the helm
//! and passes in as [`Preview`]; the create then sends that preview back unchanged, so the helm can refuse it if the
//! host's checkout folder changed since. Since the GUI sends no request keys, the old app's retained attempts for an
//! ambiguous fresh-checkout create ("Retry reconciles the original request at this path") have no counterpart here.

use farhelm_helm_client::Collections;
use farhelm_proto::conversation::validate_launch_conversation;
use farhelm_proto::launcher::{LaunchCatalogModel, TemplateNameKind, compatible_efforts};
use farhelm_proto::recipes::RecipeView;
use farhelm_proto::session_requests::{
    CreateSessionBody, EnvironmentCreateBody, ReplaceSessionBody, RestartSessionBody,
};
use farhelm_proto::tensorlake::Settings;
use farhelm_proto::ui_channel::{Availability, Collection, HostStateView, HostView};
use farhelm_proto::{
    AcceptedGithubPreview, CommandLaunch, DeleteGuard, GithubCheckoutRequest, LaunchHarness,
    LaunchPermission, LaunchSelection, LaunchTarget, RETIRED_CONTAINER_REASON, SessionLaunch,
    check_permission_flags, split_permission_flags, split_wrapper_prefix,
};

use super::draft::{
    Catalog, ENVIRONMENT_HOME, FOLDER_INSTALLATION_MOVED, Field, LaunchDraft, Mode, Restart,
    RestartBaseline, Tab, repo_identifier,
};
use crate::environment_ui::ProviderUi;
use crate::host_text::HostSentence;
use crate::hosts::offers::{HostKind, resumable};

/// What a launch is checked against besides the draft: the hosts as the store holds them, the model catalog and the
/// Tensorlake account as the launcher read them, and a managed checkout's preview.
#[derive(Clone, Copy, Debug)]
pub struct Target<'a> {
    pub hosts: &'a [HostView],
    pub catalog: Catalog<'a>,
    /// The preview of the chosen managed checkout's folder, as current for the draft as it is now. Ignored for a
    /// folder destination.
    pub preview: Preview<'a>,
    /// The recipe catalog as the store holds it, which a new environment's recipe is checked against. Ignored on an
    /// existing host, for which the view copies none ([`Recipes::for_draft`]).
    pub recipes: &'a Recipes,
    /// The Tensorlake account as the launcher read it, which a new sandbox's prerequisites are checked against. Only a
    /// provider whose account it is looks at it ([`ProviderUi::recipe_refusal`]).
    pub account: &'a Account,
}

/// The Tensorlake account as the launcher read it (`GET /api/tensorlake`, once as it opens): whether an API key is
/// saved, Farhelm's SSH key, and whether creating a sandbox skips its question. Not a pushed collection, so it is read
/// rather than followed, as Settings → Tensorlake and Add host read it once per opening; the launcher holds the one
/// copy, and every check and row reads it there.
#[derive(Clone, Debug)]
pub enum Account {
    /// The read is out.
    Reading,
    /// The read failed or could not be sent, in words already escaped for showing (the helm's are host text).
    Unreadable(String),
    /// The account as the helm answered.
    Read(Settings),
}

impl Account {
    /// The account, or the words for why there is none yet: [`LOADING_ACCOUNT`] while the read is out, else the read's
    /// own failure. What [`ProviderUi`]'s account answers take.
    pub fn settings(&self) -> Result<&Settings, &str> {
        match self {
            Account::Reading => Err(LOADING_ACCOUNT),
            Account::Unreadable(reason) => Err(reason.as_str()),
            Account::Read(settings) => Ok(settings),
        }
    }
}

/// Why a new sandbox cannot launch while the account read is out, in Add host's words for the same wait.
pub const LOADING_ACCOUNT: &str = "Loading Tensorlake settings…";

/// The pushed recipe catalog as a launch into a new environment sees it, taken once per draw or press.
#[derive(Clone, Debug)]
pub enum Recipes {
    /// The store has not installed the catalog yet.
    Loading,
    /// The helm could not read it, in its words.
    Unreadable(String),
    /// The catalog, in the helm's order.
    Listed(Vec<RecipeView>),
}

impl Recipes {
    /// What `draft`'s launch check needs of the catalog in `collections`: the catalog, for a new environment, and an
    /// empty listing for an existing host, which ignores it, so a draw or press on Existing host copies no catalog.
    pub fn for_draft(draft: &LaunchDraft, collections: &Collections) -> Recipes {
        match draft.target {
            Some(_) => Recipes::of(collections),
            None => Recipes::Listed(Vec::new()),
        }
    }

    /// The catalog as `collections` (the store's) holds it now.
    pub fn of(collections: &Collections) -> Recipes {
        match collections
            .meta(Collection::Recipes)
            .map(|meta| &meta.availability)
        {
            None => Recipes::Loading,
            Some(Availability::Unavailable { error }) => Recipes::Unreadable(error.clone()),
            Some(Availability::Available) => Recipes::Listed(
                collections
                    .ordered::<RecipeView>()
                    .into_iter()
                    .cloned()
                    .collect(),
            ),
        }
    }
}

/// Why a new environment cannot launch while the recipe catalog has not arrived, the old launcher's words.
pub const LOADING_RECIPES: &str = "Loading recipes…";
/// Why a new environment cannot launch before a recipe is chosen.
pub const CHOOSE_RECIPE: &str = "Choose a recipe before launching";
/// Why a new container cannot launch on a container host the registry no longer lists as one, the old launcher's
/// words.
pub const CONTAINER_HOST_UNAVAILABLE: &str = "The container host is unavailable";
/// The helm's own words for a container host whose Podman has not been checked yet (its places read says the same).
const NEEDS_PODMAN_CHECK: &str = "this host needs a Podman check";
/// The helm's places read's words for a host it cannot route a check to, whose entry then carries no check at all.
const CANNOT_RUN_CONTAINERS: &str = "this host cannot run containers";

/// Why `host`, a host that can hold containers ([`is_container_host`]), cannot hold a new container now, from its
/// pushed Podman check, or `None` when it can: the check's own reason once it has a result that failed, and the helm's
/// "needs a Podman check" before it has one. An entry with no check is one the helm cannot route a check to, and says
/// what the helm's places read says of it. Settings → Containers draws a host's state from the same pushed check
/// (`settings/containers.rs`).
pub fn container_reason(host: &HostView) -> Option<String> {
    let Some(check) = host.container_check.as_ref() else {
        return Some(CANNOT_RUN_CONTAINERS.to_owned());
    };
    match check.checked_at {
        Some(_) if check.usable => None,
        Some(_) => Some(
            check
                .reason
                .clone()
                .unwrap_or_else(|| NEEDS_PODMAN_CHECK.to_owned()),
        ),
        None => Some(NEEDS_PODMAN_CHECK.to_owned()),
    }
}

/// Whether `host` can hold containers at all: the helm's own machine or an SSH host, never an environment's own host
/// (SPEC.md: "an environment host cannot itself host containers"). The host kind's own answer, which Settings →
/// Containers lists its hosts by too ([`HostKind::holds_containers`]).
pub fn is_container_host(host: &HostView) -> bool {
    HostKind::of(host).holds_containers()
}

/// Where a managed checkout's preview stands for the draft as it is now: the view asks the helm again whenever what a
/// preview answers for changes (the host and its installation, the repository, the name, the agent choices), and an
/// answer for anything earlier is not one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Preview<'a> {
    /// No current answer yet: asked, about to be asked, or not askable.
    Waiting,
    /// The helm's answer: the exact folder, which the create sends back unchanged.
    Ready(&'a AcceptedGithubPreview),
    /// The helm refused to preview it, in its words, which the view shows beside the preview.
    Failed,
}

/// Why a managed checkout without a chosen repository cannot launch, the old launcher's words.
pub const SELECT_REPOSITORY: &str = "Select a GitHub repository before launching";
/// Why a named conversation cannot start in a managed checkout: a new clone has no conversation to resume. The old
/// launcher's words, which it said at the press.
pub const CONVERSATION_IN_CHECKOUT: &str =
    "Named conversations cannot start in a new managed checkout; choose an existing folder";
const CONVERSATION_WITH_PROMPT: &str =
    "This agent cannot combine an initial prompt with a named conversation";
/// Why a managed checkout waits for its preview, the old launcher's words; a failed preview says this too, while its
/// own words show beside the preview.
pub const WAIT_FOR_PREVIEW: &str = "Wait for a current checkout preview before launching";
/// Why New waits for the chosen host's launch history before launching into its untouched default folder, the old
/// launcher's words: the history decides which folder that default is (`LaunchDraft::folder_default`).
pub const FOLDER_DEFAULT_WAITING: &str = "Wait for this host's last folder before launching";
/// Why a managed checkout cannot launch on a host without an installation identity. Even a resumable host needs
/// this identity to bind the preview to the installation the user chose.
pub const UNVERIFIED_HOST: &str =
    "The host installation is not verified; a checkout cannot be created";

/// Both the launch check and its folder status require the same recorded installation.
pub(super) fn has_checkout_identity(host: &HostView) -> bool {
    host.identity.as_deref().is_some_and(|id| !id.is_empty())
}

/// An app-written reason a launch cannot start, shared by its button's help and inline refusal.
pub(super) struct Refusal(pub(super) String);

impl Refusal {
    /// Append the reason after any lead the surface needs, such as "Not launched. ".
    pub(super) fn append_to(&self, line: HostSentence) -> HostSentence {
        line.text(self.0.clone())
    }
}

impl LaunchDraft {
    /// Why the draft's place cannot take a launch now: a new environment's reasons, which take the host's place since it
    /// has no host yet, or else the chosen host's. The first of [`Self::launch_refusal`]'s reasons, and the one the launcher
    /// puts before its own waits (a `gh:` query not yet picked).
    pub(super) fn place_refusal(&self, target: Target<'_>) -> Option<Refusal> {
        match &self.target {
            Some(environment) => environment_refusal(environment, target).map(Refusal),
            None => self.host_refusal(target.hosts),
        }
    }

    /// Why this draft cannot launch now, or `None` when it can. The first reason in the old app's order wins, so the
    /// user fixes one thing at a time.
    ///
    /// `live` is the source session's stored launch as the store holds it now, for Restart with: a launch that changed
    /// since the launcher opened is refused rather than restarted with edits made against what it was (the old app's
    /// "This session's launch settings changed" check).
    pub(super) fn launch_refusal(
        &self,
        target: Target<'_>,
        live: Option<&SessionLaunch>,
    ) -> Option<Refusal> {
        self.place_refusal(target)
            .or_else(|| self.draft_refusal(target, live).map(Refusal))
    }

    /// The draft's own reasons, after the place has been found usable. None of these name host values.
    fn draft_refusal(&self, target: Target<'_>, live: Option<&SessionLaunch>) -> Option<String> {
        // The old order: the destination's reasons come right after the host's, before the launch kind's.
        if let Some(reason) = self.checkout_refusal(target) {
            return Some(reason.to_owned());
        }
        // The old order: the remembered folder's installation, then the folder default's wait.
        if self.folder_installation_moved(target.hosts) {
            return Some(FOLDER_INSTALLATION_MOVED.to_owned());
        }
        // A default for an installation the host row no longer names is about to be decided again (while drawing), so
        // it waits as a pending one does, even in the frame before that.
        if self.default_moved(target.hosts)
            || (self.folder_default_waiting()
                && !target.hosts.iter().any(|host| self.can_wake(host)))
        {
            return Some(FOLDER_DEFAULT_WAITING.to_owned());
        }
        match self.tab {
            Tab::Agent => {
                if let Some(reason) = self.agent_refusal(target.catalog) {
                    return Some(reason);
                }
            }
            Tab::Command => {
                if self.command.custom_registration_missing {
                    return Some(if self.holds(Field::AgentType) {
                        "The custom agent registration was deleted. Register it again to use Restart with."
                    } else {
                        "The custom agent registration was deleted. Choose Custom to keep using this command."
                    }.into());
                }
                if self.command.command.trim().is_empty() {
                    return Some("Enter an agent command before launching".to_owned());
                }
                if self.command.effective_yolo().is_none() {
                    return Some(
                        "Say whether this command runs in YOLO mode (YOLO) before launching"
                            .to_owned(),
                    );
                }
            }
        }
        if let Mode::RestartWith(restart) = &self.mode {
            return self.restart_refusal(restart, live);
        }
        if self.tab == Tab::Command
            && let Some(Err(error)) = self.command_launch().map(|command| command.validate())
        {
            return Some(capitalized(&error));
        }
        None
    }

    /// The request tests inspect the same wording assistive technology reads.
    #[cfg(test)]
    pub(super) fn refusal(
        &self,
        target: Target<'_>,
        live: Option<&SessionLaunch>,
    ) -> Option<String> {
        self.launch_refusal(target, live).map(|reason| {
            reason
                .append_to(HostSentence::new("test-launch-refusal"))
                .accessible_name()
                .to_string()
        })
    }

    /// The Agent tab's reasons: an agent type, flags and a wrapper that parse, choices the catalog still offers, and
    /// an initial prompt the agent can take.
    fn agent_refusal(&self, catalog: Catalog<'_>) -> Option<String> {
        let Some(selection) = self.agent_selection() else {
            return Some("Choose a structured harness before launching".to_owned());
        };
        if self.agent.permissions == Some(LaunchPermission::Custom)
            && split_permission_flags(&self.agent.permission_flags).is_err()
        {
            return Some("Enter valid custom permission flags before launching".to_owned());
        }
        if selection.harness.uses_custom_catalog() {
            if selection.custom_agent.is_none() {
                return Some("Choose a registered custom agent before launching".into());
            }
            if self.agent.custom_registration_missing {
                return Some("The selected custom agent is no longer registered".into());
            }
            for (field, value) in [
                ("model", selection.model.as_deref()),
                (
                    "effort",
                    selection.effort.as_ref().map(|value| value.as_cli_arg()),
                ),
            ] {
                if let Some(reason) = value.and_then(|value| {
                    farhelm_proto::custom_agents::validate_model_or_effort(value).err()
                }) {
                    return Some(format!("Custom agent {field}: {reason}"));
                }
            }
        } else if !selection_fits_catalog(&selection, catalog) {
            return Some(
                "This saved choice is no longer supported by the current catalog; choose a compatible model or effort"
                    .to_owned(),
            );
        }
        if self.wrapper_error().is_some() {
            return Some("Correct the wrapper prefix before launching".to_owned());
        }
        if self.agent.container.is_some() {
            return Some(RETIRED_CONTAINER_REASON.to_owned());
        }
        if let Some(prompt) = self.active_prompt()
            && let Err(error) = farhelm_proto::launcher::check_initial_prompt(prompt)
        {
            return Some(capitalized(&error));
        }
        self.named_start_error()
    }

    /// Restart with's own reasons, after the ordinary ones (SPEC.md, "Restart with": the main action is "inactive
    /// until a setting changes or a conversation selector is entered").
    fn restart_refusal(&self, restart: &Restart, live: Option<&SessionLaunch>) -> Option<String> {
        let baseline = &restart.baseline;
        if self.requires_conversation() && self.active_conversation().is_none() {
            return Some("Enter a conversation to restart".to_owned());
        }
        if self.restart_edit().as_ref() == Some(baseline) && self.active_conversation().is_none() {
            return Some("Change a setting before restarting".to_owned());
        }
        if let (RestartBaseline::Command(_), Some(Err(_))) = (
            baseline,
            self.command_launch().map(|command| command.validate()),
        ) {
            return Some("Correct the command before restarting".to_owned());
        }
        if live != Some(&restart.opened) {
            return Some(
                "This session's launch settings changed; close and reopen restart with".to_owned(),
            );
        }
        None
    }

    /// A managed checkout needs a repository, a verified existing installation and no named conversation. A
    /// connected host needs its current preview. A resumable host can take the press before there is a connection on
    /// which to ask for that preview.
    fn checkout_refusal(&self, target: Target<'_>) -> Option<&'static str> {
        let checkout = self.checkout.as_ref()?;
        if checkout.repo.is_none() {
            return Some(SELECT_REPOSITORY);
        }
        if self.conversation_in_checkout() {
            return Some(CONVERSATION_IN_CHECKOUT);
        }
        // A new environment has no installation to verify and no folder to preview yet (the module docs).
        if self.target.is_some() {
            return None;
        }
        let host = self
            .host
            .and_then(|id| target.hosts.iter().find(|host| host.id == id))
            .filter(|host| has_checkout_identity(host));
        let Some(host) = host else {
            return Some(UNVERIFIED_HOST);
        };
        if self.can_wake(host) {
            return None;
        }
        match target.preview {
            Preview::Ready(_) => None,
            Preview::Waiting | Preview::Failed => Some(WAIT_FOR_PREVIEW),
        }
    }

    /// A press can wake an existing environment in a resumable phase. An ordinary disconnected host still needs its
    /// own recovery first, and a new environment launch has no existing destination to resume.
    pub(super) fn can_wake(&self, host: &HostView) -> bool {
        self.target.is_none()
            && self.host == Some(host.id)
            && resumable(&host.state)
            && HostKind::of(host).provider().is_some()
    }

    /// The target host's refusal, if it is neither connected nor available to wake. Restart with holds its source's
    /// host, so its remedy cannot ask the user to select another.
    pub(super) fn host_refusal(&self, hosts: &[HostView]) -> Option<Refusal> {
        let view = self
            .host
            .and_then(|id| hosts.iter().find(|host| host.id == id));
        let words = match (&self.mode, view) {
            (_, Some(host))
                if matches!(host.state, HostStateView::Connected { .. }) || self.can_wake(host) =>
            {
                None
            }
            (Mode::RestartWith(_), None) => Some(
                "The source host installation cannot be confirmed; close and reopen restart with"
                    .to_owned(),
            ),
            (Mode::RestartWith(_), Some(_)) => Some(
                "The source host is unavailable; wait for it to reconnect before restarting"
                    .to_owned(),
            ),
            (Mode::New | Mode::Clone | Mode::ReplaceWith { .. }, _) => {
                Some("Choose a connected host before launching".to_owned())
            }
        };
        words.map(Refusal)
    }

    /// Request builders require an actual connection even when the press is allowed to wake the host.
    fn connected_host<'a>(&self, hosts: &'a [HostView]) -> Option<&'a HostView> {
        hosts.iter().find(|host| {
            self.host == Some(host.id) && matches!(host.state, HostStateView::Connected { .. })
        })
    }

    /// The Agent tab as the launch it would send: `None` without an agent type. The flags and the wrapper are taken
    /// only when they parse ([`Self::launch_refusal`] has already refused text that does not), and a permission the agent
    /// does not offer becomes its default, except in Restart with, which keeps the stored permission as it was built.
    pub fn agent_selection(&self) -> Option<LaunchSelection> {
        let agent = &self.agent;
        let harness = agent.harness?;
        let permissions = if matches!(self.mode, Mode::RestartWith(_)) {
            agent.permissions
        } else {
            harness.request_permission(agent.permissions)
        };
        let permission_flags = (permissions == Some(LaunchPermission::Custom))
            .then(|| split_permission_flags(&agent.permission_flags).ok())
            .flatten();
        let wrapper_prefix = (!agent.wrapper.is_empty())
            .then(|| split_wrapper_prefix(&agent.wrapper).ok())
            .flatten();
        Some(LaunchSelection {
            custom_agent: agent.custom_agent.clone(),
            harness,
            model: agent.model.clone(),
            effort: agent.effort_value(),
            permissions,
            permission_flags,
            workspace_trust: None,
            wrapper_prefix,
            container: agent.container.clone(),
        })
    }

    /// The Command tab as the launch it would send: `None` until the YOLO question is answered. The resume command
    /// counts only while Resume is on and an agent type is declared.
    pub fn command_launch(&self) -> Option<CommandLaunch> {
        let command = &self.command;
        Some(CommandLaunch {
            custom_agent: command.custom_agent.clone(),
            command: command.command.clone(),
            yolo: command.effective_yolo()?,
            agent: command.agent,
            resume: (command.resume_on && command.agent.is_some()).then(|| command.resume.clone()),
        })
    }

    /// The initial prompt this launch sends: only on the Agent tab, only when the agent type can take one (or none is
    /// chosen yet), and only when it is not empty. Text kept while that does not hold stays in the draft unsent.
    pub fn active_prompt(&self) -> Option<&str> {
        let supported = self
            .agent
            .harness
            .is_none_or(|harness| harness.supports_initial_prompt(false));
        (self.tab == Tab::Agent && supported && !self.prompt.is_empty())
            .then_some(self.prompt.as_str())
    }

    /// The conversation selector this launch sends: only on the Agent tab, for an agent that offers one, when filled
    /// in.
    pub(super) fn active_conversation(&self) -> Option<&str> {
        (self.tab == Tab::Agent && self.offers_conversation() && !self.conversation.is_empty())
            .then_some(self.conversation.as_str())
    }

    /// A name can only be acted on by a registration with a resume command.
    pub(super) fn offers_conversation(&self) -> bool {
        self.agent.harness.is_some_and(|harness| {
            harness.offers_named_conversation()
                && (!harness.uses_custom_catalog() || self.agent.custom_resumption)
        })
    }

    /// The restart could not acquire a captured conversation. Supplying a valid selector makes it actionable,
    /// but the setting remains required for this restart and must stay visible.
    pub(super) fn requires_conversation(&self) -> bool {
        matches!(&self.mode, Mode::RestartWith(restart) if restart.conversation_required)
    }

    /// A saved or typed wrapper uses the same parser for the field warning and the launch prerequisite.
    pub(super) fn wrapper_error(&self) -> Option<String> {
        if self.agent.wrapper.is_empty() {
            return None;
        }
        split_wrapper_prefix(&self.agent.wrapper).err()
    }

    /// The field warning uses the checkout prerequisite even while an earlier refusal takes precedence.
    pub(super) fn conversation_error(&self) -> Option<String> {
        if self.conversation_in_checkout() {
            return Some(CONVERSATION_IN_CHECKOUT.to_owned());
        }
        self.named_start_error()
    }

    /// A selected checkout destination conflicts with a named start even before a repository has been picked.
    fn conversation_in_checkout(&self) -> bool {
        self.checkout.is_some() && self.active_conversation().is_some()
    }

    /// Apply the helm's literal selector boundary before submission. A valid selector can still conflict with a
    /// prompt on an agent that only accepts prompts for fresh starts.
    fn named_start_error(&self) -> Option<String> {
        if self.tab == Tab::Agent
            && self
                .agent
                .harness
                .is_some_and(LaunchHarness::uses_custom_catalog)
            && !self.conversation.is_empty()
            && !self.agent.custom_resumption
        {
            return Some(if self.requires_conversation() {
                "This custom agent no longer supports resumption."
            } else {
                "This custom agent cannot resume now. Clear the conversation ID to start a new conversation."
            }.into());
        }
        let conversation = self.active_conversation()?;
        let harness = self.agent.harness?;
        if let Err(error) = validate_launch_conversation(harness, conversation) {
            return Some(capitalized(&error));
        }
        (self.active_prompt().is_some() && !harness.supports_initial_prompt(true))
            .then(|| CONVERSATION_WITH_PROMPT.to_owned())
    }

    /// Whether the YOLO question's "Start guarded session instead" would still change anything: in Restart with, a
    /// guarded launch equal to the stored one is no edit, and a restart then has nothing to send (SPEC.md, "Creation":
    /// the guarded answer is offered "for Restart with only when the result still differs from the stored launch"; the
    /// old draft's `restart_guarded_changes`). Every other mode starts something new, so the answer always changes it.
    pub fn guarded_still_changes(&self) -> bool {
        let Mode::RestartWith(restart) = &self.mode else {
            return true;
        };
        // A named conversation is a change of its own, which a guarded restart keeps (the old rule counted it too).
        if self.active_conversation().is_some() {
            return true;
        }
        let mut guarded = self.clone();
        guarded.choose_permissions(Some(LaunchPermission::Guarded));
        guarded.restart_edit().as_ref() != Some(&restart.baseline)
    }

    /// Restart with's edit, in the baseline's own shape, for comparing with the baseline and for the request.
    fn restart_edit(&self) -> Option<RestartBaseline> {
        let Mode::RestartWith(restart) = &self.mode else {
            return None;
        };
        match restart.baseline {
            RestartBaseline::Agent(_) => self.agent_selection().map(RestartBaseline::Agent),
            RestartBaseline::Command(_) => self.command_launch().map(RestartBaseline::Command),
        }
    }

    // -----------------------------------------------------------------------------------------------------------------
    // Request bodies
    // -----------------------------------------------------------------------------------------------------------------

    /// The create this draft sends (`POST /api/sessions`), for New and Clone; `None` while [`Self::launch_refusal`] names a
    /// reason. The YOLO confirmation is the caller's to set, after the user answers the host's question. Replace with
    /// sends this create inside [`Self::replace_body`]; sending it alone would create without deleting the source.
    pub fn create_body(&self, target: Target<'_>) -> Option<CreateSessionBody> {
        // A new environment's launch goes to its own route ([`Self::environment_body`]), never the ordinary create.
        if self.target.is_some() || self.launch_refusal(target, None).is_some() {
            return None;
        }
        let host = self.connected_host(target.hosts)?;
        let (cwd, name, expected_incarnation, github_checkout) = match self.checkout_launch(target)
        {
            // A managed checkout starts in the previewed folder, on the connection the preview was taken on, with the
            // name its folder is named after (empty for an unnamed one, which the helm names `<repo>-N`).
            Some((repo, preview)) => {
                let title = self.checkout_title();
                (
                    preview.binding.cwd.clone(),
                    title.clone(),
                    Some(preview.incarnation),
                    Some(GithubCheckoutRequest {
                        repo,
                        title: Some(title),
                        preview: Some(preview.clone()),
                    }),
                )
            }
            None => (
                self.folder.value().clone(),
                self.name.value().clone(),
                Some(host.incarnation),
                None,
            ),
        };
        let mut body = self.create_request(cwd, name);
        body.host = Some(host.id);
        body.expected_incarnation = expected_incarnation;
        body.github_checkout = github_checkout;
        Some(body)
    }

    /// The create's launch, prompt, conversation and name as this draft holds them, starting in `cwd` with the name
    /// `name`, and naming no host, connection or checkout: the part the ordinary create and a new environment's share.
    fn create_request(&self, cwd: String, name: String) -> CreateSessionBody {
        let (launch, command) = match self.tab {
            Tab::Agent => (self.agent_selection(), None),
            Tab::Command => (None, self.command_launch()),
        };
        CreateSessionBody {
            host: None,
            expected_incarnation: None,
            cwd,
            // A blank name asks for a generated one; any other name is sent byte for byte (old
            // `optional_create_title` at the reference commit).
            title: (!name.trim().is_empty()).then_some(name),
            launch,
            command,
            prompt: self.active_prompt().map(str::to_owned),
            conversation: self.active_conversation().map(str::to_owned),
            confirm_yolo: false,
            github_checkout: None,
        }
    }

    /// The create in a new environment this draft sends (`POST /api/environment-sessions`), for New and Clone with a
    /// recipe chosen; `None` without one, or while [`Self::launch_refusal`] names a reason. The recipe and place go beside
    /// the create, which names no host or connection (the environment does not exist yet) and starts in the
    /// environment's home. `confirmed`, the answer to "Create a sandbox for this session?", is the caller's to set.
    pub fn environment_body(&self, target: Target<'_>) -> Option<EnvironmentCreateBody> {
        let LaunchTarget::Environment { recipe, place } = self.target.clone()?;
        if self.launch_refusal(target, None).is_some() {
            return None;
        }
        let repo = self
            .checkout
            .as_ref()
            .and_then(|checkout| checkout.repo.as_ref());
        let request = match repo {
            // A managed checkout goes with no folder and no preview: the helm previews it once the new environment's
            // Farhelm connects, under the name the checkout gets (the old launcher's body, `cwd` empty).
            Some(repo) => {
                let title = self.checkout_title();
                let mut request = self.create_request(String::new(), title.clone());
                request.github_checkout = Some(GithubCheckoutRequest {
                    repo: repo_identifier(repo),
                    title: Some(title),
                    preview: None,
                });
                request
            }
            None => self.create_request(ENVIRONMENT_HOME.to_owned(), self.name.value().clone()),
        };
        // A numbered default name (a template's base, or Clone's `<title>-clone`) is the helm's to number once it can
        // see what is taken there.
        let name_kind = match repo.and_then(|_| self.numbered_base()) {
            Some(_) => Some(TemplateNameKind::Base),
            None => self.name_kind,
        };
        Some(EnvironmentCreateBody {
            recipe,
            place,
            confirmed: false,
            confirmed_size: None,
            name_kind,
            request,
        })
    }

    /// A managed checkout's repository and current preview, for the create: `None` for a folder destination.
    /// [`Self::launch_refusal`] has already refused a checkout without both.
    fn checkout_launch<'a>(
        &self,
        target: Target<'a>,
    ) -> Option<(String, &'a AcceptedGithubPreview)> {
        let repo = self.checkout.as_ref()?.repo.as_ref()?;
        match target.preview {
            Preview::Ready(preview) => Some((repo_identifier(repo), preview)),
            Preview::Waiting | Preview::Failed => None,
        }
    }

    /// Replace with's request (`POST /api/sessions/{id}/replace`): this draft's create, then the source's delete
    /// under the precondition matching what the launcher showed about the source at the press (SPEC.md, "Replace
    /// with": "its launch button is the confirmation of that delete").
    pub fn replace_body(
        &self,
        target: Target<'_>,
        guard: DeleteGuard,
    ) -> Option<ReplaceSessionBody> {
        let with = self.create_body(target)?;
        Some(ReplaceSessionBody {
            confirm_yolo: false,
            with: Some(with),
            only_if_nothing_alive: guard.only_if_nothing_alive(),
            only_if_agent_ended: guard.only_if_agent_ended(),
        })
    }

    /// Restart with's request (`POST /api/sessions/{id}/restart`): the edited launch in the baseline's shape, and the
    /// conversation the user named, if any. `stop_if_running` says the user confirmed stopping a working agent ("Stop
    /// and restart").
    /// Pin the routed connection at dispatch, as a create does. A verified YOLO answer can first follow a reconnect
    /// to the same installation, then the retry pins that newly observed connection.
    pub fn restart_body(
        &self,
        target: Target<'_>,
        live: Option<&SessionLaunch>,
        stop_if_running: bool,
    ) -> Option<RestartSessionBody> {
        if self.launch_refusal(target, live).is_some() {
            return None;
        }
        let host = self.connected_host(target.hosts)?;
        let (with, with_command) = match self.restart_edit()? {
            RestartBaseline::Agent(selection) => (Some(selection), None),
            RestartBaseline::Command(command) => (None, Some(command)),
        };
        Some(RestartSessionBody {
            expected_incarnation: Some(host.incarnation),
            conversation: self.active_conversation().map(str::to_owned),
            stop_if_running,
            with,
            with_command,
            confirm_yolo: false,
        })
    }
}

/// A new environment's reason, if it has one, in the old launcher's order: the catalog has arrived and can be read, it
/// lists the recipe, the provider can run it with the account as read ([`ProviderUi::recipe_refusal`]: a sandbox's API
/// key, SSH key and snapshot), and a container's host can hold it.
fn environment_refusal(environment: &LaunchTarget, target: Target<'_>) -> Option<String> {
    let recipes = match target.recipes {
        Recipes::Loading => return Some(LOADING_RECIPES.to_owned()),
        // The read's own words, as the old launcher showed them.
        Recipes::Unreadable(error) => return Some(error.clone()),
        Recipes::Listed(recipes) => recipes,
    };
    let name = environment.recipe();
    // Entering a segment while the catalog lists no recipe the provider can run leaves the recipe unchosen.
    if name.is_empty() {
        return Some(CHOOSE_RECIPE.to_owned());
    }
    let Some(recipe) = recipes.iter().find(|recipe| recipe.name == name) else {
        return Some(format!("Recipe \"{name}\" is unavailable"));
    };
    if let Some(reason) =
        ProviderUi(environment.provider()).recipe_refusal(recipe, target.account.settings())
    {
        return Some(reason);
    }
    // A place on a container host needs that host; another place has no host of the user's to check.
    let id = environment.place().container_host()?;
    match target
        .hosts
        .iter()
        .find(|host| host.id == id && is_container_host(host))
    {
        None => Some(CONTAINER_HOST_UNAVAILABLE.to_owned()),
        Some(host) => container_reason(host),
    }
}

/// Whether `selection` still fits what the catalog offers. Without a catalog read, nothing the catalog decides
/// (which models an agent owns, which efforts a model takes) refuses a choice; what needs no catalog is still checked
/// (old `selection_fits_catalog` and `selection_is_compatible` in `launch_composer.rs` at the reference commit).
fn selection_fits_catalog(selection: &LaunchSelection, catalog: Catalog<'_>) -> bool {
    match catalog {
        Some(catalog) => selection_is_compatible(selection, catalog),
        None => {
            (selection.effort.is_none() || selection.harness.offers_effort())
                && selection_is_compatible(
                    &LaunchSelection {
                        effort: None,
                        ..selection.clone()
                    },
                    &[],
                )
        }
    }
}

/// The catalog-dependent half of [`selection_fits_catalog`]: flags that fit their permission, no model or effort on an
/// agent without that choice, an effort the model or agent offers, and a permission the agent offers. An older
/// model's spelling can still be a custom ID on another agent. A current model's spelling needs one of its owners.
fn selection_is_compatible(selection: &LaunchSelection, catalog: &[LaunchCatalogModel]) -> bool {
    if check_permission_flags(selection.permissions, selection.permission_flags.as_deref()).is_err()
    {
        return false;
    }
    if !selection.harness.offers_model()
        && (selection.model.is_some() || selection.effort.is_some())
    {
        return false;
    }
    // Read through the agent's own catalog spelling, as the helm does: a bare OpenCode name is OpenCode's model.
    let mut owned = None;
    if let Some(model) = &selection.model {
        let id = selection.harness.catalog_model_id(model);
        let mut known = catalog.iter().filter(|candidate| candidate.id == id);
        owned = known
            .clone()
            .find(|model| model.harness == selection.harness);
        if owned.is_none() && known.any(|model| model.latest) {
            return false;
        }
    }
    let effort_fits = selection.effort.as_ref().is_none_or(|value| {
        value.known().is_some_and(|effort| match owned {
            Some(model) => model.efforts.contains(&effort),
            None => compatible_efforts(selection.harness, None, catalog).contains(&effort),
        })
    });
    let permission_fits = selection
        .permissions
        .is_none_or(|permission| selection.harness.offers_permission(permission));
    effort_fits && permission_fits
}

/// A shared rule's error as the launcher shows it: the same words with a capital first letter.
pub(super) fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

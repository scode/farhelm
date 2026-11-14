//! From a draft to a launch: whether it can launch, why not, and the request body it sends.
//!
//! SPEC.md, "Creation": "an incomplete attempt says why beside Launch, as does a refused Launch press, and the
//! greyed-out button's hover names what is missing". [`LaunchDraft::refusal`] is that one reason, the first that
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
//! at the press. A suspended host is not launchable yet: resuming it before the create (SPEC.md, "Topology")
//! comes with the launcher's environments work.
//!
//! The bodies are `farhelm_proto::session_requests`' types, built only from a draft that passes
//! [`LaunchDraft::refusal`]. What the old app's bodies carried for its retry keys is gone: the GUI sends none
//! (SPEC.md, "Creation").

use farhelm_proto::launcher::{LaunchCatalogModel, compatible_efforts};
use farhelm_proto::session_requests::{CreateSessionBody, ReplaceSessionBody, RestartSessionBody};
use farhelm_proto::ui_channel::{HostStateView, HostView};
use farhelm_proto::{
    CommandLaunch, DeleteGuard, LaunchPermission, LaunchSelection, RETIRED_CONTAINER_REASON,
    SessionLaunch, check_permission_flags, split_permission_flags, split_wrapper_prefix,
};

use super::draft::{Catalog, LaunchDraft, Mode, Restart, RestartBaseline, Tab};

/// What a launch is checked against besides the draft: the hosts as the store holds them, and the model catalog as
/// the launcher read it.
#[derive(Clone, Copy, Debug)]
pub struct Target<'a> {
    pub hosts: &'a [HostView],
    pub catalog: Catalog<'a>,
}

impl LaunchDraft {
    /// Why this draft cannot launch now, or `None` when it can. The first reason in the old app's order wins, so the
    /// user fixes one thing at a time.
    ///
    /// `live` is the source session's stored launch as the store holds it now, for Restart with: a launch that changed
    /// since the launcher opened is refused rather than restarted with edits made against what it was (the old app's
    /// "This session's launch settings changed" check).
    pub fn refusal(&self, target: Target<'_>, live: Option<&SessionLaunch>) -> Option<String> {
        if let Some(reason) = self.host_refusal(target.hosts) {
            return Some(reason);
        }
        match self.tab {
            Tab::Agent => {
                if let Some(reason) = self.agent_refusal(target.catalog) {
                    return Some(reason);
                }
            }
            Tab::Command => {
                if self.command.command.trim().is_empty() {
                    return Some("Enter an agent command before launching".to_owned());
                }
                if self.command.yolo.is_none() {
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
        if !selection_fits_catalog(&selection, catalog) {
            return Some(
                "This saved choice is no longer supported by the current catalog; choose a compatible model or effort"
                    .to_owned(),
            );
        }
        if !self.agent.wrapper.is_empty() && split_wrapper_prefix(&self.agent.wrapper).is_err() {
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
        if self.active_prompt().is_some()
            && self.active_conversation().is_some()
            && !selection.harness.supports_initial_prompt(true)
        {
            return Some(
                "This agent cannot combine an initial prompt with a named conversation".to_owned(),
            );
        }
        None
    }

    /// Restart with's own reasons, after the ordinary ones (SPEC.md, "Restart with": the main action is "inactive
    /// until a setting changes or a conversation selector is entered").
    fn restart_refusal(&self, restart: &Restart, live: Option<&SessionLaunch>) -> Option<String> {
        let baseline = &restart.baseline;
        if restart.conversation_required && self.conversation.is_empty() {
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

    /// The target host's reason, if it has one: it must be in the store and connected (SPEC.md: the host picker lists
    /// unavailable hosts with their phase, and a create against one cannot go anywhere). Restart with holds the
    /// source's host, so its wording names that host rather than asking for another (the old app's words).
    fn host_refusal(&self, hosts: &[HostView]) -> Option<String> {
        let view = self
            .host
            .and_then(|id| hosts.iter().find(|host| host.id == id));
        let connected =
            view.is_some_and(|host| matches!(host.state, HostStateView::Connected { .. }));
        match (&self.mode, view) {
            (_, Some(_)) if connected => None,
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
        }
    }

    /// The Agent tab as the launch it would send: `None` without an agent type. The flags and the wrapper are taken
    /// only when they parse ([`Self::refusal`] has already refused text that does not), and a permission the agent
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
            harness,
            model: agent.model.clone(),
            effort: agent.effort,
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
            command: command.command.clone(),
            yolo: command.yolo?,
            agent: command.agent,
            resume: (command.resume_on && command.agent.is_some()).then(|| command.resume.clone()),
        })
    }

    /// The initial prompt this launch sends: only on the Agent tab, only when the agent type can take one (or none is
    /// chosen yet), and only when it is not empty. Text kept while that does not hold stays in the draft unsent.
    fn active_prompt(&self) -> Option<&str> {
        let supported = self
            .agent
            .harness
            .is_none_or(|harness| harness.supports_initial_prompt(false));
        (self.tab == Tab::Agent && supported && !self.prompt.is_empty())
            .then_some(self.prompt.as_str())
    }

    /// The conversation selector this launch sends: only on the Agent tab, for an agent that offers one, when filled
    /// in.
    fn active_conversation(&self) -> Option<&str> {
        let offered = self
            .agent
            .harness
            .is_some_and(|harness| harness.offers_named_conversation());
        (self.tab == Tab::Agent && offered && !self.conversation.is_empty())
            .then_some(self.conversation.as_str())
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

    /// The create this draft sends (`POST /api/sessions`), for New and Clone; `None` while [`Self::refusal`] names a
    /// reason. The YOLO confirmation is the caller's to set, after the user answers the host's question. Replace with
    /// sends this create inside [`Self::replace_body`]; sending it alone would create without deleting the source.
    pub fn create_body(&self, target: Target<'_>) -> Option<CreateSessionBody> {
        if self.refusal(target, None).is_some() {
            return None;
        }
        let (launch, command) = match self.tab {
            Tab::Agent => (self.agent_selection(), None),
            Tab::Command => (None, self.command_launch()),
        };
        let host = self.host?;
        let name = self.name.value();
        Some(CreateSessionBody {
            host: Some(host),
            // The refusal check has found this host connected, so its connection number is a real one.
            expected_incarnation: target
                .hosts
                .iter()
                .find(|view| view.id == host)
                .map(|view| view.incarnation),
            cwd: self.folder.value().clone(),
            // A blank name asks for a generated one; any other name is sent byte for byte (old
            // `optional_create_title` at the reference commit).
            title: (!name.trim().is_empty()).then(|| name.clone()),
            launch,
            command,
            prompt: self.active_prompt().map(str::to_owned),
            conversation: self.active_conversation().map(str::to_owned),
            confirm_yolo: false,
            github_checkout: None,
        })
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
    pub fn restart_body(
        &self,
        target: Target<'_>,
        live: Option<&SessionLaunch>,
        stop_if_running: bool,
    ) -> Option<RestartSessionBody> {
        if self.refusal(target, live).is_some() {
            return None;
        }
        let (with, with_command) = match self.restart_edit()? {
            RestartBaseline::Agent(selection) => (Some(selection), None),
            RestartBaseline::Command(command) => (None, Some(command)),
        };
        Some(RestartSessionBody {
            conversation: self.active_conversation().map(str::to_owned),
            stop_if_running,
            with,
            with_command,
            confirm_yolo: false,
        })
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
/// agent without that choice, a known model owned by this agent, an effort the model (or the agent) offers, and a
/// permission the agent offers.
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
    let known: Option<Vec<&LaunchCatalogModel>> = selection.model.as_ref().map(|model| {
        let id = selection.harness.catalog_model_id(model);
        catalog
            .iter()
            .filter(|candidate| candidate.id == id)
            .collect()
    });
    let owned = known.as_ref().and_then(|models| {
        models
            .iter()
            .find(|model| model.harness == selection.harness)
            .copied()
    });
    if known.as_ref().is_some_and(|models| !models.is_empty()) && owned.is_none() {
        return false;
    }
    let effort_fits = selection.effort.is_none_or(|effort| match owned {
        Some(model) => model.efforts.contains(&effort),
        None => compatible_efforts(selection.harness, None, catalog).contains(&effort),
    });
    let permission_fits = selection
        .permissions
        .is_none_or(|permission| selection.harness.offers_permission(permission));
    effort_fits && permission_fits
}

/// A shared rule's error as the launcher shows it: the same words with a capital first letter.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

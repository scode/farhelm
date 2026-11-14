//! The launcher's draft: everything the session launcher holds while it is open, and every edit to it.
//!
//! SPEC.md, "Creation" and "Lifecycle operations". Framework-free on purpose (no GPUI type in here, tested without a
//! window): the launcher's view draws a [`LaunchDraft`] and turns each control's input into one of its edit methods.
//! The old app's equivalent was `create_form/draft.rs` with `launch_composer.rs` at reference commit `24bc0580`; the
//! rules shared with the helm (reconciling an agent switch, efforts per model, which permissions an agent offers) are
//! `farhelm_proto`'s and are called, never copied.
//!
//! Here's the TLDR of the shape:
//!
//! - Two tabs, each with its own fields ([`AgentChoices`], [`CommandChoices`]); switching tabs never copies one into
//!   the other, so each keeps its draft (SPEC.md: "switching tabs keeps each tab's draft"). The initial prompt and the
//!   conversation selector are one-time inputs beside them.
//! - A destination: host, folder and session name. The folder and the name are [`Draft`]s, because their touched mark
//!   decides whether something else may overwrite them: a host change re-seeds only a folder the user has not typed or
//!   picked (SPEC.md, "Working directory").
//! - A [`Mode`]: New, Clone, Replace with or Restart with. Clone, Replace with and Restart with are seeded from the
//!   source session's stored launch, verbatim, never parsed back from a command line (SPEC.md, "Clone"). Seeding is a
//!   separate path from editing: it marks nothing touched and raises no banner. [`LaunchDraft::holds`] answers which
//!   fields a mode holds fixed.
//! - Every hand edit is one method (`choose_*`, `edit_*`) that applies the edit with all its side effects:
//!   reconciliation on an agent switch, the remembered wrapper prefix for the new agent, clearing what no longer fits,
//!   and the permission-escalation detection the red banner shows. These methods are the launcher's seam for anything
//!   that edits the draft the way a hand edit does: the composer search's results use them, and so will applying a
//!   launch template (SPEC.md, "Launch templates": applying one makes "the edits it contains, in the same way and with
//!   the same effects as making them by hand").
//!
//! The draft is a snapshot of the source taken when the launcher opens, as the old app's was: a session that changes
//! underneath an open Clone does not reseed it. The helm validates every launch again; nothing here turns a model id
//! into argv.
//!
//! Not here: the composer search's parsing, recent setups and the host's default-folder precedence (the view passes
//! the folder a host change should seed), managed checkouts, environments, and the launch's validation and request
//! bodies.

use farhelm_proto::launcher::{
    LaunchCatalogModel, compatible_efforts, reconcile_harness_selection,
};
use farhelm_proto::{
    CommandLaunch, LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, SessionInfo,
    SessionLaunch,
};

use crate::drafts::Draft;

/// The release's model catalog as the launcher read it: `None` until `GET /api/launch-catalog` answers (or when it
/// failed), which is not the same as an empty catalog. A missing catalog never makes a choice incompatible; the old
/// app refused catalog-dependent choices only against a catalog it had actually read (`launch_composer.rs`,
/// `reconcile_harness_for_catalog_read` at the reference commit).
pub type Catalog<'a> = Option<&'a [LaunchCatalogModel]>;

/// The launcher's two launch kinds, shown as the Agent and Command tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Agent,
    Command,
}

/// What the launcher was opened for, which decides what it was seeded from, what it holds fixed and what launching
/// does (create; create then delete the source; restart the source).
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    New,
    Clone,
    /// Replace with: a create on the source's own host, then the source's delete.
    ReplaceWith {
        source: String,
    },
    /// Restart with: the source relaunched with edited settings, resuming its own or a named conversation. Boxed
    /// because it carries two launches and the other modes almost nothing.
    RestartWith(Box<Restart>),
}

/// What Restart with keeps about its source.
#[derive(Clone, Debug, PartialEq)]
pub struct Restart {
    pub source: String,
    pub baseline: RestartBaseline,
    /// The source has a stored resume command but no captured conversation, so this restart must name one (SPEC.md,
    /// "Restart with": "its conversation field is required and explains why").
    pub conversation_required: bool,
    /// The source's whole stored launch when the launcher opened, which a restart is refused against if the session's
    /// launch changed meanwhile (the old app compared the whole launch, not only its editable part).
    pub opened: SessionLaunch,
}

/// The source's launch as Restart with opened it, which an edit is compared against: the main action stays inactive
/// until a setting differs from it (SPEC.md, "Restart with").
#[derive(Clone, Debug, PartialEq)]
pub enum RestartBaseline {
    /// The agent's choices, with the permission the stored command was actually built with
    /// ([`LaunchHarness::stored_permission`]) rather than today's default for an omission.
    Agent(LaunchSelection),
    Command(CommandLaunch),
}

impl RestartBaseline {
    /// The baseline Restart with edits for `launch`, or `None` when Restart with is not offered for it: an agent launch
    /// without a stored resume command (Cursor, OpenCode, an old Muse launch), a command launch that did not declare an
    /// agent type and opt into Resume, and a launch from before launch kinds. SPEC.md, "Restart with"; old
    /// `RestartEdit::baseline` (`api.rs` at the reference commit).
    pub fn of(launch: &SessionLaunch) -> Option<RestartBaseline> {
        match launch {
            SessionLaunch::Agent {
                selection,
                resume: Some(_),
                ..
            } => {
                let mut selection = selection.clone();
                selection.permissions = selection.harness.stored_permission(selection.permissions);
                selection.workspace_trust = None;
                Some(RestartBaseline::Agent(selection))
            }
            SessionLaunch::Command(command)
                if command.agent.is_some() && command.resume.is_some() =>
            {
                Some(RestartBaseline::Command(command.clone()))
            }
            SessionLaunch::Agent { resume: None, .. }
            | SessionLaunch::Command(_)
            | SessionLaunch::Legacy { .. } => None,
        }
    }
}

/// The fields a mode can hold fixed: their controls are hidden, and anything that would set one (a search result, a
/// recent setup, a template) is refused even when it names the current value (SPEC.md, "Restart with").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    LaunchKind,
    AgentType,
    Host,
    Destination,
    Name,
    InitialPrompt,
}

/// The Agent tab's choices, as the user made them. Text fields keep their raw text even while it does not parse, so a
/// half-typed flag list is never lost; parsing happens when the launch is checked.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgentChoices {
    /// The agent type; New starts with none.
    pub harness: Option<LaunchHarness>,
    /// The accepted model id; `None` is the agent type's configured default.
    pub model: Option<String>,
    /// Which agent type a custom (non-catalog) model id was entered for, so a later agent switch clears it exactly as
    /// [`reconcile_harness_selection`] expects.
    pub model_owner: Option<LaunchHarness>,
    /// `None` is the agent type's default effort.
    pub effort: Option<LaunchEffort>,
    /// The permission as chosen. An agent switch replaces a permission the new agent does not offer with its default
    /// (the shared reconciliation), so switching back does not restore it, as in the old app.
    pub permissions: Option<LaunchPermission>,
    /// Whether `permissions` was chosen deliberately, rather than being the remembered value New starts with. Only a
    /// deliberate choice made before any agent can be "escalated" by choosing the first agent (SPEC.md: "a displayed
    /// default or a remembered permission alone does not").
    pub permissions_explicit: bool,
    /// The Custom flags editor's raw text.
    pub permission_flags: String,
    /// The wrapper prefix editor's raw text, loaded from the agent type's remembered prefix on an agent switch.
    pub wrapper: String,
    /// A retired container choice carried by a session from before containerized sessions were retired. It is kept
    /// only so that launching from it is refused: such a session supports Delete only (SPEC.md, "Retired containerized
    /// sessions"), and dropping the choice would quietly relaunch it as an ordinary session.
    pub container: Option<farhelm_proto::ContainerConfig>,
}

/// The Command tab's fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandChoices {
    pub command: String,
    /// The required YOLO assertion; `None` until the user answers it, which is not the same as "no".
    pub yolo: Option<bool>,
    /// The optional declared agent type.
    pub agent: Option<LaunchHarness>,
    /// Whether the user opted into Resume; it counts only while an agent type is declared.
    pub resume_on: bool,
    /// The resume command's raw text, kept while Resume is off or hidden.
    pub resume: String,
}

/// What an agent-choice edit did besides changing the draft, for the view to show.
///
/// How the view uses it, as the old app did: the notice replaces whatever notice the agent controls showed (none
/// clears it, and so do [`LaunchDraft::choose_default_model`] and [`LaunchDraft::reset_choices`], which return no
/// `Edit`); when `switched_agent` is set, a banner already showing goes away (its Undo would restore a draft from
/// before an earlier switch) and `escalation`, if any, is the new one; an edit that did not switch agents leaves a
/// showing banner alone.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edit {
    /// A deliberate choice the edit cleared or replaced, or why the edit was refused, said beside the agent controls
    /// ("The selected effort is not in Farhelm's offering for that model, so it was cleared").
    pub notice: Option<String>,
    /// The edit changed the agent type.
    pub switched_agent: bool,
    /// The edit made the shown permission more dangerous by switching agents: the red banner's content.
    pub escalation: Option<Escalation>,
}

/// Restart with's refusal of an edit that would change the agent type or the launch kind it holds (the old app's
/// words).
const AGENT_TYPE_HELD: &str =
    "Agent type cannot change here: this dialog keeps the session's agent type";

/// An agent switch that made the shown permission more dangerous (SPEC.md, "Creation": the red banner naming both
/// modes, with Undo restoring the draft before the switch).
#[derive(Clone, Debug, PartialEq)]
pub struct Escalation {
    pub before: Option<LaunchPermission>,
    pub after: Option<LaunchPermission>,
    pub harness: LaunchHarness,
    /// The whole draft as it was before the switch, which Undo puts back.
    pub undo: Box<LaunchDraft>,
}

impl Escalation {
    /// The banner's sentence, as the old app worded it (`create_form.rs` at the reference commit).
    pub fn message(&self) -> String {
        format!(
            "Permission changed: {} → {}, because {} does not support {}.",
            permission_name(self.before),
            permission_name(self.after),
            harness_label(self.harness),
            permission_name(self.before),
        )
    }
}

/// Everything the launcher holds. See the module docs.
///
/// The fields are public for the view to read; every change goes through the edit methods, which carry each edit's
/// side effects.
#[derive(Clone, Debug, PartialEq)]
pub struct LaunchDraft {
    pub mode: Mode,
    pub tab: Tab,
    pub agent: AgentChoices,
    pub command: CommandChoices,
    /// The initial prompt's text; kept while the active tab or agent type cannot send it, and sent only where it can.
    pub prompt: String,
    /// The one-time conversation selector, passed literally; empty is a fresh start.
    pub conversation: String,
    /// The target host's registry id; `None` only while the helm has reported no host at all.
    pub host: Option<i64>,
    pub folder: Draft<String>,
    pub name: Draft<String>,
}

// ---------------------------------------------------------------------------------------------------------------------
// Seeding
// ---------------------------------------------------------------------------------------------------------------------

impl LaunchDraft {
    /// New: no agent type, the Agent tab, and the remembered permission as a passive default (SPEC.md: "The
    /// permissions mode remembers the last successful agent launch"). The caller supplies New's host and folder
    /// (SPEC.md, "Host" and "Working directory").
    pub fn new(
        host: Option<i64>,
        folder: String,
        remembered: Option<LaunchPermission>,
    ) -> LaunchDraft {
        LaunchDraft {
            mode: Mode::New,
            tab: Tab::Agent,
            agent: AgentChoices {
                permissions: remembered,
                ..AgentChoices::default()
            },
            command: CommandChoices::default(),
            prompt: String::new(),
            conversation: String::new(),
            host,
            folder: Draft::load(folder),
            name: Draft::load(String::new()),
        }
    }

    /// Clone of `session` on `host` (the caller resolves which host row still names the session's install): its
    /// folder, title and stored launch, with the initial prompt cleared and the conversation selector empty, so the
    /// clone starts fresh (SPEC.md, "Clone").
    pub fn clone_of(session: &SessionInfo, host: Option<i64>) -> LaunchDraft {
        let mut draft = LaunchDraft::seeded(Mode::Clone, session, host);
        draft.seed_launch(&session.launch);
        draft
    }

    /// Replace with `source`: as Clone, but on the source's own host, which the mode holds (SPEC.md, "Replace with").
    pub fn replace_with(source: &str, session: &SessionInfo, host: Option<i64>) -> LaunchDraft {
        let mut draft = LaunchDraft::seeded(
            Mode::ReplaceWith {
                source: source.to_owned(),
            },
            session,
            host,
        );
        draft.seed_launch(&session.launch);
        draft
    }

    /// Restart with `source`, or `None` when its launch offers no Restart with ([`RestartBaseline::of`]). The agent
    /// tab shows the baseline's choices with the permission the stored command was built with; the command tab shows
    /// the stored command and resume command.
    pub fn restart_with(
        source: &str,
        session: &SessionInfo,
        host: Option<i64>,
    ) -> Option<LaunchDraft> {
        let baseline = RestartBaseline::of(&session.launch)?;
        let conversation_required = matches!(baseline, RestartBaseline::Agent(_))
            && session.restart_offer != farhelm_proto::RestartOffer::Resume;
        let mut draft = LaunchDraft::seeded(
            Mode::RestartWith(Box::new(Restart {
                source: source.to_owned(),
                baseline: baseline.clone(),
                conversation_required,
                opened: session.launch.clone(),
            })),
            session,
            host,
        );
        match &baseline {
            RestartBaseline::Agent(selection) => {
                draft.restore_selection(selection);
                draft.agent.permissions = selection.permissions;
            }
            RestartBaseline::Command(command) => draft.seed_command(command),
        }
        Some(draft)
    }

    /// The parts every source seeding shares: the source's folder and title, untouched, and its command line on the
    /// Command tab, so an agent session's Clone can be turned into a command launch of the same command (the old app
    /// seeded `invocation` there too). A command launch's own fields replace it.
    fn seeded(mode: Mode, session: &SessionInfo, host: Option<i64>) -> LaunchDraft {
        LaunchDraft {
            mode,
            tab: Tab::Agent,
            agent: AgentChoices::default(),
            command: CommandChoices {
                command: session.invocation.clone(),
                ..CommandChoices::default()
            },
            prompt: String::new(),
            conversation: String::new(),
            host,
            folder: Draft::load(session.cwd.clone()),
            name: Draft::load(session.title.clone()),
        }
    }

    /// Seed the tabs from a stored launch: an agent launch's choices on the Agent tab, or a command launch's fields
    /// on the Command tab. A launch from before launch kinds has no structured choices; its stored command is the
    /// only thing that carries over, with the YOLO question left for the user to answer.
    fn seed_launch(&mut self, launch: &SessionLaunch) {
        match launch {
            SessionLaunch::Agent { selection, .. } => self.restore_selection(selection),
            SessionLaunch::Command(command) => self.seed_command(command),
            SessionLaunch::Legacy { invocation, .. } => {
                self.command.command = invocation.clone();
                self.tab = Tab::Command;
                self.agent.permissions_explicit = true;
            }
        }
    }

    /// An agent launch's stored choices, verbatim, as deliberate ones (a restored launch is the user's explicit
    /// selection, so its permission is never treated as a passive default).
    fn restore_selection(&mut self, selection: &LaunchSelection) {
        self.tab = Tab::Agent;
        self.agent = AgentChoices {
            harness: Some(selection.harness),
            model: selection.model.clone(),
            model_owner: selection.model.as_ref().map(|_| selection.harness),
            effort: selection.effort,
            permissions: selection.harness.request_permission(selection.permissions),
            permissions_explicit: true,
            // Flags are literal words, so their editor text quotes them back; a wrapper's words keep the quotes and
            // escapes they were typed with, so they are only joined (SPEC.md, "Launch composer").
            permission_flags: selection
                .permission_flags
                .as_deref()
                .map(farhelm_proto::join_permission_flags)
                .unwrap_or_default(),
            wrapper: joined(selection.wrapper_prefix.as_deref()),
            container: selection.container.clone(),
        };
    }

    /// A command launch's stored fields; its agent tab stays empty.
    fn seed_command(&mut self, command: &CommandLaunch) {
        self.tab = Tab::Command;
        self.agent.permissions_explicit = true;
        self.command = CommandChoices {
            command: command.command.clone(),
            yolo: Some(command.yolo),
            agent: command.agent,
            resume_on: command.resume.is_some(),
            resume: command.resume.clone().unwrap_or_default(),
        };
    }

    /// Whether this mode holds `field` fixed. A presence check, not an equality one: Restart with refuses an attempt
    /// to set a held field even when it names the current value (SPEC.md, "Restart with").
    pub fn holds(&self, field: Field) -> bool {
        match &self.mode {
            Mode::New | Mode::Clone => false,
            Mode::ReplaceWith { .. } => field == Field::Host,
            Mode::RestartWith(_) => true,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Agent tab edits
// ---------------------------------------------------------------------------------------------------------------------

impl LaunchDraft {
    /// Switch to the Agent or Command tab, keeping both tabs' drafts. Restart with holds the launch kind, so there it
    /// changes nothing (its tabs are hidden; this guards a search result or template that would set the tab).
    pub fn choose_tab(&mut self, tab: Tab) {
        if !self.holds(Field::LaunchKind) {
            self.tab = tab;
        }
    }

    /// Choose `harness` as the agent type: the choices made for another type are reconciled (incompatible ones
    /// cleared, with a notice for any the user had chosen deliberately), the new type's remembered wrapper prefix
    /// loads, Custom flags never cross agents, and the tab becomes Agent. `remembered_wrapper` is the prefix the helm
    /// remembers for `harness` (`Preferences::remembered_wrapper_prefixes`), as shell words. In Restart with, which
    /// holds the agent type and the launch kind, anything but the same agent on the Agent tab is refused.
    pub fn choose_harness(
        &mut self,
        harness: LaunchHarness,
        catalog: Catalog<'_>,
        remembered_wrapper: Option<&[String]>,
    ) -> Edit {
        let selection = self.choices_for(harness);
        let owner = self.agent.model_owner;
        self.switch_agent(selection, owner, harness, catalog, remembered_wrapper)
    }

    /// Choose the catalog model `id`, which `owner` offers. Within the current agent type it only clears an effort the
    /// model does not offer; a model of another type (picked from the full model list) switches the agent type on
    /// purpose, with everything [`Self::choose_harness`] does (SPEC.md: "choosing another harness's model from the full
    /// model list switches the harness on purpose").
    pub fn choose_catalog_model(
        &mut self,
        owner: LaunchHarness,
        id: &str,
        catalog: Catalog<'_>,
        remembered_wrapper: Option<&[String]>,
    ) -> Edit {
        if self.agent.harness != Some(owner) {
            // The picked model goes into the reconciliation, so it is the one kept and nothing reports the old one
            // as cleared (the old app's `choose_catalog_model`).
            let mut selection = self.choices_for(owner);
            selection.model = Some(id.to_owned());
            // A catalog model has no custom owner. Passed in rather than written to the draft, so a refused switch, and
            // the Undo snapshot taken before it, keep the draft exactly as it was.
            return self.switch_agent(selection, None, owner, catalog, remembered_wrapper);
        }
        self.agent.model = Some(id.to_owned());
        self.agent.model_owner = None;
        Edit {
            notice: self
                .clear_incompatible_effort(owner, catalog)
                .then(|| EFFORT_CLEARED.to_owned()),
            ..Edit::default()
        }
    }

    /// Move the agent choices in `selection` to `harness`, with every effect of a hand switch: reconciliation and its
    /// notice, the new type's remembered wrapper prefix, Custom flags dropped, the Agent tab, and the escalation check.
    /// `model_owner` is which agent `selection`'s model was typed for, if it is a custom id.
    fn switch_agent(
        &mut self,
        selection: LaunchSelection,
        model_owner: Option<LaunchHarness>,
        harness: LaunchHarness,
        catalog: Catalog<'_>,
        remembered_wrapper: Option<&[String]>,
    ) -> Edit {
        if self.holds(Field::AgentType)
            && (self.agent.harness != Some(harness) || self.tab != Tab::Agent)
        {
            return Edit {
                notice: Some(AGENT_TYPE_HELD.to_owned()),
                ..Edit::default()
            };
        }
        let before = self.clone();
        let notice = self.reconcile(selection, model_owner, harness, catalog);
        let switched_agent = before.agent.harness != Some(harness);
        if switched_agent {
            self.agent.wrapper = joined(remembered_wrapper);
            self.agent.permission_flags = String::new();
        }
        self.tab = Tab::Agent;
        Edit {
            notice,
            switched_agent,
            escalation: self.escalation_since(&before),
        }
    }

    /// A custom model id typed into the model field for the chosen agent type, which never changes the type (SPEC.md:
    /// "Typing a model never changes a selected harness"). The caller has already resolved Enter to a custom id; with
    /// no agent type chosen there is nothing to own it, so nothing changes.
    pub fn choose_custom_model(&mut self, id: &str, catalog: Catalog<'_>) -> Edit {
        let Some(harness) = self.agent.harness else {
            return Edit::default();
        };
        self.agent.model = Some(id.to_owned());
        self.agent.model_owner = Some(harness);
        Edit {
            notice: self.clear_incompatible_effort(harness, catalog).then(|| {
                "The selected effort is not in Farhelm's offering for that model or harness, so it was cleared".to_owned()
            }),
            ..Edit::default()
        }
    }

    /// The agent type's configured default model; the effort stays.
    pub fn choose_default_model(&mut self) {
        self.agent.model = None;
        self.agent.model_owner = None;
    }

    /// `None` is the default effort.
    pub fn choose_effort(&mut self, effort: Option<LaunchEffort>) {
        self.agent.effort = effort;
    }

    /// A permission chosen directly. It never raises the escalation banner (SPEC.md: "Choosing a permission directly
    /// never triggers it"), and leaving Custom drops its flags, which belong to that choice alone.
    pub fn choose_permissions(&mut self, permissions: Option<LaunchPermission>) {
        if permissions != Some(LaunchPermission::Custom) {
            self.agent.permission_flags.clear();
        }
        self.agent.permissions = permissions;
        self.agent.permissions_explicit = true;
    }

    /// The Custom flags editor's text, kept as typed even while it does not parse.
    pub fn edit_permission_flags(&mut self, text: String) {
        self.agent.permission_flags = text;
        self.agent.permissions_explicit = true;
    }

    pub fn edit_wrapper(&mut self, text: String) {
        self.agent.wrapper = text;
    }

    pub fn edit_conversation(&mut self, text: String) {
        self.conversation = text;
    }

    pub fn edit_prompt(&mut self, text: String) {
        self.prompt = text;
    }

    /// Reset choices: New's agent choices again, with permissions back to the remembered value; in Restart with, the
    /// source's own launch. The command tab, the one-time inputs and the destination stay (old `reset_choices` at the
    /// reference commit; SPEC.md: "Reset choices returns permissions and the wrapper prefix to their remembered
    /// values").
    pub fn reset_choices(&mut self, remembered: Option<LaunchPermission>) {
        if let Mode::RestartWith(restart) = &self.mode
            && let RestartBaseline::Agent(selection) = &restart.baseline
        {
            let selection = selection.clone();
            self.restore_selection(&selection);
            self.agent.permissions = selection.permissions;
            self.conversation.clear();
            return;
        }
        self.agent = AgentChoices {
            permissions: remembered,
            ..AgentChoices::default()
        };
    }

    /// The agent tab's choices as a selection on `harness`, which [`reconcile_harness_selection`] takes. The flags
    /// are parsed leniently (text that does not parse is no choice to carry across); the wrapper is left out, because a
    /// switch replaces it with the new type's remembered prefix rather than reconciling it.
    fn choices_for(&self, harness: LaunchHarness) -> LaunchSelection {
        let agent = &self.agent;
        LaunchSelection {
            harness: agent.harness.unwrap_or(harness),
            model: agent.model.clone(),
            effort: agent.effort,
            permissions: agent.permissions,
            permission_flags: farhelm_proto::split_permission_flags(&agent.permission_flags).ok(),
            workspace_trust: None,
            wrapper_prefix: None,
            container: None,
        }
    }

    /// Move the agent choices to `harness` through the shared reconciliation, and say which deliberate choice it
    /// cleared. A passive remembered permission is never reported as cleared.
    fn reconcile(
        &mut self,
        selection: LaunchSelection,
        model_owner: Option<LaunchHarness>,
        harness: LaunchHarness,
        catalog: Catalog<'_>,
    ) -> Option<String> {
        let effort = selection.effort;
        let (mut after, owner) = reconcile_harness_selection(
            selection.clone(),
            model_owner,
            harness,
            catalog.unwrap_or(&[]),
        );
        if catalog.is_none() && harness.offers_effort() {
            // An unread catalog cannot make an effort incompatible.
            after.effort = effort;
        }
        let mut deliberate = selection;
        if !self.agent.permissions_explicit {
            deliberate.permissions = None;
        }
        let notice = reconciliation_notice(&deliberate, &after);
        self.agent.harness = Some(after.harness);
        self.agent.model = after.model;
        self.agent.model_owner = owner;
        self.agent.effort = after.effort;
        self.agent.permissions = after.permissions;
        notice
    }

    /// Clear an effort the current model of `harness` does not offer, against a catalog that was read. Returns whether
    /// it cleared one.
    fn clear_incompatible_effort(&mut self, harness: LaunchHarness, catalog: Catalog<'_>) -> bool {
        let (Some(effort), Some(catalog)) = (self.agent.effort, catalog) else {
            return false;
        };
        if compatible_efforts(harness, self.agent.model.as_deref(), catalog).contains(&effort) {
            return false;
        }
        self.agent.effort = None;
        true
    }

    /// Whether switching agents since `before` made the shown permission more dangerous (SPEC.md: "Chat and approve
    /// are safest, then smart approve and Guarded, then YOLO, custom flags and the agent's own setting; only a move to
    /// a later group triggers the banner"). Before the first agent, only a permission chosen deliberately counts. A
    /// switch made from the Command tab, or onto it, raises nothing.
    ///
    /// Public because a batch of edits that counts as one action compares once, from before the whole batch: applying
    /// a template is judged after all its edits (SPEC_impl.md, "Launch templates"), not edit by edit.
    pub fn escalation_since(&self, before: &LaunchDraft) -> Option<Escalation> {
        let harness = self.agent.harness?;
        if before.agent.harness == Some(harness)
            || before.tab != Tab::Agent
            || self.tab != Tab::Agent
        {
            return None;
        }
        let from = match before.agent.harness {
            Some(previous) => previous.request_permission(before.agent.permissions),
            None if before.agent.permissions_explicit => before.agent.permissions,
            None => None,
        };
        let to = harness.request_permission(self.agent.permissions);
        // With no previous agent and no deliberate choice, `from` is the agent's own setting, the most dangerous
        // group, so nothing can rise above it: a displayed default never raises the banner.
        (danger(to) > danger(from)).then(|| Escalation {
            before: from,
            after: to,
            harness,
            undo: Box::new(before.clone()),
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Command tab and destination edits
// ---------------------------------------------------------------------------------------------------------------------

impl LaunchDraft {
    pub fn edit_command(&mut self, text: String) {
        self.command.command = text;
    }

    pub fn choose_command_yolo(&mut self, yolo: bool) {
        self.command.yolo = Some(yolo);
    }

    /// The declared agent type; clearing it hides Resume but keeps the resume command's text.
    pub fn choose_command_agent(&mut self, agent: Option<LaunchHarness>) {
        self.command.agent = agent;
    }

    pub fn choose_command_resume(&mut self, on: bool) {
        self.command.resume_on = on;
    }

    pub fn edit_command_resume(&mut self, text: String) {
        self.command.resume = text;
    }

    /// Choose `host` as the target. `default_folder` is the folder New would seed for it, which replaces the folder
    /// only while the user has not typed or picked one in this draft (SPEC.md, "Working directory": "Changing the host
    /// re-seeds a folder the user has not typed or picked in that draft"). Choosing the host already chosen changes
    /// nothing, so it cannot re-seed a Clone's folder.
    pub fn choose_host(&mut self, host: i64, default_folder: String) {
        if self.host == Some(host) {
            return;
        }
        self.host = Some(host);
        self.seed_folder(default_folder);
    }

    /// The folder New seeds for the chosen host, which replaces the folder only while the user has not typed or
    /// picked one. Apart from [`Self::choose_host`] so a default that arrives later (the host's recent folders read
    /// after the host was chosen) seeds it the same way.
    pub fn seed_folder(&mut self, default_folder: String) {
        let _ = self.folder.follow(default_folder);
    }

    /// The folder field's text, as typed, or a folder picked from the dropdown, Browse or search: either is the
    /// user's own choice, so a later host change keeps it.
    pub fn edit_folder(&mut self, text: String) {
        // `Draft::edit` refuses only while a save of the draft is in flight; the launcher never saves its drafts (a
        // launch is not a save), so every edit lands.
        let _ = self.folder.edit(text);
    }

    pub fn edit_name(&mut self, text: String) {
        let _ = self.name.edit(text);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Wording shared with the launcher's view
// ---------------------------------------------------------------------------------------------------------------------

/// The permission New starts with, from the helm's remembered word (`Preferences::remembered_permissions`). Custom is
/// never remembered (SPEC.md: "Custom never becomes the remembered default permission"), and a word this build does
/// not know, written by a newer helm, starts with none rather than guessing (old `initial_structured_permissions`).
pub fn remembered_permission(word: Option<&str>) -> Option<LaunchPermission> {
    match word {
        Some("yolo") => Some(LaunchPermission::Yolo),
        Some("guarded") => Some(LaunchPermission::Guarded),
        Some("approve") => Some(LaunchPermission::Approve),
        Some("smart_approve") => Some(LaunchPermission::SmartApprove),
        Some("chat") => Some(LaunchPermission::Chat),
        _ => None,
    }
}

/// The notice for an effort a model choice cleared.
const EFFORT_CLEARED: &str =
    "The selected effort is not in Farhelm's offering for that model, so it was cleared";

/// The agent type's name as the launcher shows it.
pub const fn harness_label(harness: LaunchHarness) -> &'static str {
    match harness {
        LaunchHarness::Codex => "Codex",
        LaunchHarness::Claude => "Claude",
        LaunchHarness::Muse => "Muse",
        LaunchHarness::Cursor => "Cursor",
        LaunchHarness::Grok => "Grok",
        LaunchHarness::Goose => "Goose",
        LaunchHarness::Pi => "Pi",
        LaunchHarness::Omp => "OMP",
        LaunchHarness::OpenCode => "OpenCode",
    }
}

/// A permission's name in the escalation banner; `None` is the agent's own setting.
pub const fn permission_name(permission: Option<LaunchPermission>) -> &'static str {
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

/// SPEC.md's safety groups: 0 asks about actions, 1 lets a safety mechanism approve them, 2 has no check (or one
/// Farhelm cannot see: the agent's own setting, custom flags).
const fn danger(permission: Option<LaunchPermission>) -> u8 {
    match permission {
        Some(LaunchPermission::Chat | LaunchPermission::Approve) => 0,
        Some(LaunchPermission::SmartApprove | LaunchPermission::Guarded) => 1,
        Some(LaunchPermission::Yolo | LaunchPermission::Custom) | None => 2,
    }
}

/// What an agent switch cleared of the user's deliberate choices, in one sentence, or `None` when it cleared nothing
/// (old `reconciliation_reset_reason` in `launch_composer.rs` at the reference commit).
fn reconciliation_notice(before: &LaunchSelection, after: &LaunchSelection) -> Option<String> {
    let mut cleared = Vec::new();
    if before.model.is_some() && after.model.is_none() {
        cleared.push("selected model is not in this harness's Farhelm offering");
    }
    if before.effort.is_some() && after.effort.is_none() {
        cleared.push("selected effort is not in Farhelm's offering for that model or harness");
    }
    let mut notices = Vec::new();
    if !cleared.is_empty() {
        notices.push(format!("The {}, so it was cleared", cleared.join("; the ")));
    }
    if let Some(permission) = before.permissions
        && !after.harness.offers_permission(permission)
    {
        let disposition = match after.permissions {
            Some(LaunchPermission::Yolo) => "replaced by YOLO, this harness's default",
            Some(LaunchPermission::Guarded) => "replaced by guarded, this harness's default",
            Some(
                LaunchPermission::Approve
                | LaunchPermission::SmartApprove
                | LaunchPermission::Chat
                | LaunchPermission::Custom,
            ) => "replaced by this harness's supported mode",
            None => "cleared",
        };
        // Clauses share one sentence; only the first starts with a capital.
        let article = if notices.is_empty() { "The" } else { "the" };
        notices.push(format!(
            "{article} selected permission is unavailable for this harness, so it was {disposition}"
        ));
    }
    (!notices.is_empty()).then(|| notices.join("; "))
}

/// A wrapper prefix's raw words as its editor shows them: joined with spaces, quotes and escapes kept as stored.
fn joined(words: Option<&[String]>) -> String {
    words.map(|words| words.join(" ")).unwrap_or_default()
}

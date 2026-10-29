//! Accepted tab drafts, placement and one-time inputs, independent of widgets.
//!
//! Raw editor text and ownership belong together: parsing a draft and writing
//! it back would lose unfinished quotes and change untouched peer values.
//! Preview observations, confirmation and picker state stay with the form.
//! `LaunchDraftState` shares the form's existing host, operation lock and retry
//! handles for all editing and finite source/naming reconciliation;
//! submission still owns mint/reply lifetime.
//! Field memos are subscriptions, never live callback authority.
//!
//! Ordinary edit facts describe submitted changes, not admission or assignment:
//! false still performs ownership, explicitness and dormant-buffer writes.
//! Malformed affected word lists conservatively report change. Host/history
//! authority is passed through existing handles. Concrete operations select
//! their fixed rules; private template replay never recursively publishes or
//! retires a pair. Permission recovery stores immutable values, not edit code.

use dioxus::prelude::*;

use crate::github_checkout::{GithubPreview, PreviewAuthority};
use crate::ops::OpLock;
use crate::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, api};

use super::{
    BrowseAuthority, CheckoutNameSource, CheckoutNaming, CloneHostAction, CloneHostState,
    CommandLaunch, CreatePrefill, CreateTarget, DestinationDraft, FolderChoice,
    GithubCheckoutRequest, GithubRepo, HostId, HostOption, IntentBinding, LaunchIntent, LaunchTab,
    PermissionEscalation, PermissionSwitchBefore, SharedPreferences, active_prompt,
    admit_history_destination, browse_activation_is_current, command_intent, display_peer,
    effective_title, history_target, initial_structured_permissions, invalidate_directory_browse,
    promote_history_snapshot, remembered_destination_matches, remembered_wrapper,
    resolve_clone_host, submitted_field,
};

/// External or deliberately frozen facts for one envelope projection.
/// The host pair comes from the selected row initially and the binding after
/// minting; a retained binding must not acquire a newer row's incarnation.
/// These borrows last only for assembly, never as another mutable state owner.
/// Supplying them does not establish that submission admission has succeeded.
pub(super) struct RequestContext<'a> {
    pub(super) host: Option<(HostId, &'a str)>,
    pub(super) launch: LaunchIntent,
    pub(super) replace_source: Option<&'a str>,
    pub(super) checkout: Option<CheckoutInput<'a>>,
}

/// A new admitted preview and a previously accepted checkout have different
/// title contracts. Refreshing a binding may change its title without rewriting
/// the accepted checkout's original title, path or preview/configuration facts.
pub(super) enum CheckoutInput<'a> {
    Preview {
        repo: &'a GithubRepo,
        preview: &'a GithubPreview,
    },
    Accepted {
        cwd: &'a str,
        request: &'a GithubCheckoutRequest,
    },
}

/// Accepted launch choices and their raw-text ownership, retained across tabs.
///
/// This is a draft, not a validated request: malformed custom flags and wrapper
/// text must remain available for correction. The request projection does not
/// replace the form's ordered admission checks. Preview replies and host
/// authority stay outside semantic destination ownership. Both tab drafts and
/// one-time inputs survive mode changes; request() uses the caller's frozen
/// launch variant even if later edits have selected a different tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LaunchDraft {
    /// Restart compares against stored choices; it must never acquire create's
    /// permission defaults or permit a search action to replace held context.
    restart_baseline: Option<api::RestartEdit>,
    harness: Option<LaunchHarness>,
    model: Option<String>,
    model_raw_seed: Option<String>,
    model_edited: bool,
    model_owner: Option<LaunchHarness>,
    effort: Option<LaunchEffort>,
    permissions: Option<LaunchPermission>,
    permissions_explicit: bool,
    permission_flags: String,
    wrapper: String,
    wrapper_explicit: bool,
    tab: LaunchTab,
    invocation: String,
    invocation_raw_seed: Option<String>,
    invocation_edited: bool,
    command_yolo: Option<bool>,
    command_agent: Option<LaunchHarness>,
    command_resume_on: bool,
    command_resume: String,
    command_resume_raw_seed: Option<String>,
    command_resume_edited: bool,
    prompt: Option<String>,
    conversation: String,
    cwd: String,
    cwd_raw_seed: Option<String>,
    cwd_edited: bool,
    folder_is_explicit: bool,
    title: String,
    title_raw_seed: Option<String>,
    title_edited: bool,
    destination: DestinationDraft,
    checkout_naming: CheckoutNaming,
}

/// Preserve the existing control/search differences while sharing transitions.
/// Search restores model ownership like a relayed choice; the model control
/// accepts deliberately edited text. Their compatibility notices also differ.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ChoiceOrigin {
    Control,
    Search,
}

/// Compatibility feedback and the submitted change made by an agent choice.
/// A missing notice does not imply equality: model bytes or permission defaults
/// can change silently. Callers separately compose tab/path/authority changes.
pub(super) struct AgentChoiceOutcome {
    pub(super) reset_reason: Option<String>,
    pub(super) selection_changed: bool,
}

impl LaunchDraft {
    /// Prefill reconciliation copies Restart's stored baseline once per
    /// opening. Edits retain it, so its presence also names the draft's held
    /// mode without a second independently mutable flag.
    pub(super) fn is_restart(&self) -> bool {
        self.restart_baseline.is_some()
    }

    /// Seed only remembered permissions; New never guesses an agent or model.
    pub(super) fn new(remembered_permission: Option<LaunchPermission>) -> Self {
        Self {
            restart_baseline: None,
            harness: None,
            model: None,
            model_raw_seed: None,
            model_edited: false,
            model_owner: None,
            effort: None,
            permissions: remembered_permission,
            permissions_explicit: false,
            permission_flags: String::new(),
            wrapper: String::new(),
            wrapper_explicit: false,
            tab: LaunchTab::Agent,
            invocation: String::new(),
            invocation_raw_seed: None,
            invocation_edited: false,
            command_yolo: None,
            command_agent: None,
            command_resume_on: false,
            command_resume: String::new(),
            command_resume_raw_seed: None,
            command_resume_edited: false,
            prompt: None,
            conversation: String::new(),
            cwd: "~".into(),
            cwd_raw_seed: None,
            cwd_edited: false,
            folder_is_explicit: false,
            title: String::new(),
            title_raw_seed: None,
            title_edited: false,
            destination: DestinationDraft::Existing { cwd: "~".into() },
            checkout_naming: CheckoutNaming::default(),
        }
    }

    /// The accepted harness, independently of the form's active tab.
    pub(super) fn harness(&self) -> Option<LaunchHarness> {
        self.harness
    }
    /// The accepted model spelling; unaccepted picker text is not stored here.
    pub(super) fn model(&self) -> Option<String> {
        self.model.clone()
    }
    /// Preserve inherited model bytes separately from their editable spelling.
    pub(super) fn model_raw_seed(&self) -> Option<String> {
        self.model_raw_seed.clone()
    }
    /// Whether model text was deliberately accepted as an edit.
    pub(super) fn model_edited(&self) -> bool {
        self.model_edited
    }
    /// A custom ID may move only within the harness that owns it.
    pub(super) fn model_owner(&self) -> Option<LaunchHarness> {
        self.model_owner
    }
    /// The accepted effort, including an explicit return to harness default.
    pub(super) fn effort(&self) -> Option<LaunchEffort> {
        self.effort
    }
    /// Raw permission choice before submission normalizes harness defaults.
    pub(super) fn permissions(&self) -> Option<LaunchPermission> {
        self.permissions
    }
    /// Passive memory must not filter recents or preselect a template field.
    pub(super) fn permissions_explicit(&self) -> bool {
        self.permissions_explicit
    }
    /// Raw custom editor text survives spacing and quote errors unchanged.
    pub(super) fn permission_flags(&self) -> String {
        self.permission_flags.clone()
    }
    /// Raw wrapper editor text, not a reconstructed parsed command line.
    pub(super) fn wrapper(&self) -> String {
        self.wrapper.clone()
    }
    /// Remembered wrappers are visible without becoming explicit template edits.
    pub(super) fn wrapper_explicit(&self) -> bool {
        self.wrapper_explicit
    }

    /// The active launch kind; each group's dormant values remain intact.
    pub(super) fn tab(&self) -> LaunchTab {
        self.tab
    }

    /// Editable command spelling, which may differ from inherited bytes.
    pub(super) fn invocation(&self) -> String {
        self.invocation.clone()
    }

    /// Untouched source bytes; reading these does not imply editor ownership.
    pub(super) fn invocation_raw_seed(&self) -> Option<String> {
        self.invocation_raw_seed.clone()
    }

    /// A deliberate edit owns even an unchanged escaped display spelling.
    pub(super) fn invocation_edited(&self) -> bool {
        self.invocation_edited
    }

    /// The required command assertion, including its unanswered state.
    pub(super) fn command_yolo(&self) -> Option<bool> {
        self.command_yolo
    }

    /// Command metadata, independent of the structured agent choice.
    pub(super) fn command_agent(&self) -> Option<LaunchHarness> {
        self.command_agent
    }

    /// Resume opt-in; disabling it preserves dormant editor values.
    pub(super) fn command_resume_on(&self) -> bool {
        self.command_resume_on
    }

    /// Editable resume spelling, retained while its controls are hidden.
    pub(super) fn command_resume(&self) -> String {
        self.command_resume.clone()
    }

    /// Original resume bytes retained independently of their display spelling.
    pub(super) fn command_resume_raw_seed(&self) -> Option<String> {
        self.command_resume_raw_seed.clone()
    }

    /// Whether the resume editor deliberately owns its current text.
    pub(super) fn command_resume_edited(&self) -> bool {
        self.command_resume_edited
    }

    /// One-time raw prompt; template Some("") differs from an absent prompt.
    pub(super) fn prompt(&self) -> Option<String> {
        self.prompt.clone()
    }

    /// One-time literal selector, excluded from reusable launch choices.
    pub(super) fn conversation(&self) -> String {
        self.conversation.clone()
    }

    // Placement ownership. Existing.cwd is the operational destination, while
    // the editor triple survives managed mode for an explicit return to Folder.
    // All folder transitions keep those roles linked without caching previews.

    /// Dormant folder display survives managed mode; it is never a preview path.
    pub(super) fn cwd(&self) -> String {
        self.cwd.clone()
    }
    /// Untouched folder requests retain peer bytes rather than their display.
    pub(super) fn cwd_raw_seed(&self) -> Option<String> {
        self.cwd_raw_seed.clone()
    }
    /// Typing claims even a display spelling identical to its escaped seed.
    pub(super) fn cwd_edited(&self) -> bool {
        self.cwd_edited
    }
    /// Picking a raw path is explicit placement without claiming its display.
    pub(super) fn folder_is_explicit(&self) -> bool {
        self.folder_is_explicit
    }
    /// Name editor display, independently of a resolved checkout suffix.
    pub(super) fn title(&self) -> String {
        self.title.clone()
    }
    /// Copied title ownership survives a temporary managed destination.
    pub(super) fn title_raw_seed(&self) -> Option<String> {
        self.title_raw_seed.clone()
    }
    /// Whether the name is the user's exact spelling rather than a copied seed.
    pub(super) fn title_edited(&self) -> bool {
        self.title_edited
    }
    /// Semantic destination only; preview completion cannot rewrite this value.
    pub(super) fn destination(&self) -> DestinationDraft {
        self.destination.clone()
    }
    /// Existing name source and bounded occupancy sequence, without authority.
    pub(super) fn checkout_naming(&self) -> CheckoutNaming {
        self.checkout_naming.clone()
    }
    /// Operational folder bytes for live browse and ordinary request checks.
    pub(super) fn submitted_cwd(&self) -> String {
        submitted_field(&self.cwd, self.cwd_edited, self.cwd_raw_seed.as_deref())
    }
    /// Shared name policy for snapshots that must not wait on renderer memos.
    pub(super) fn effective_title(&self) -> String {
        effective_title(
            &self.title,
            self.title_edited,
            self.title_raw_seed.as_deref(),
            &self.destination,
            &self.checkout_naming,
        )
    }
    // Plain placement transitions do not decide host/history admission, preview
    // generations, focus or retry policy. Concrete owner operations compose
    // those existing boundaries; search/template replay retains its own order.

    /// A typed folder exits managed mode and deliberately owns its spelling.
    /// Report operational path change, independently of editor ownership bits.
    pub(super) fn edit_folder(&mut self, text: String) -> bool {
        let changed =
            !matches!(&self.destination, DestinationDraft::Existing { cwd } if cwd == &text);
        self.destination = DestinationDraft::Existing { cwd: text.clone() };
        self.cwd = text;
        self.cwd_edited = true;
        self.folder_is_explicit = true;
        changed
    }
    /// Relayed path choices preserve exact bytes and count as explicit placement.
    /// Equality concerns the semantic destination, not raw/edited ownership;
    /// callers separately account for invalid remembered-destination correction.
    pub(super) fn choose_existing_folder(&mut self, raw: &str) -> bool {
        let changed =
            !matches!(&self.destination, DestinationDraft::Existing { cwd } if cwd == raw);
        self.seed_folder(raw);
        self.folder_is_explicit = true;
        changed
    }
    /// Inherited placement is visible without preselecting the template checkbox.
    pub(super) fn seed_folder(&mut self, raw: &str) {
        self.destination = DestinationDraft::Existing { cwd: raw.into() };
        self.cwd = display_peer(raw);
        self.cwd_raw_seed = Some(raw.into());
        self.cwd_edited = false;
        self.folder_is_explicit = false;
    }
    /// Mode changes retain the dormant editor; queries never become folders.
    /// The owner refuses same-mode choices before ancillary work; plain callers
    /// such as template replay retain their own complete transaction policy.
    pub(super) fn choose_destination_type(&mut self, managed: bool) {
        if managed {
            self.edit_repository_query(String::new());
        } else {
            self.choose_existing_folder(&self.submitted_cwd());
        }
    }
    /// Query text has no repository-selection or request authority.
    pub(super) fn edit_repository_query(&mut self, query: String) {
        self.destination = DestinationDraft::GithubInput { query };
    }
    /// Callers separately invalidate preview revision on every explicit choice.
    pub(super) fn select_repository(&mut self, repo: GithubRepo) {
        self.destination = DestinationDraft::github(repo);
    }
    /// Hand edits replace Clone/template naming while retaining dormant raw bytes.
    /// Ordinary retry identity follows the API's optional-name rule; managed
    /// naming keeps its existing conservative retirement and preview contract.
    pub(super) fn edit_title(&mut self, text: String) -> bool {
        let before = self.effective_title();
        self.checkout_naming = CheckoutNaming::default();
        self.title = text;
        self.title_edited = true;
        !matches!(self.destination, DestinationDraft::Existing { .. })
            || api::optional_create_title(&before)
                != api::optional_create_title(&self.effective_title())
    }
    /// Clone alone supplies a source repository; Replace keeps its copied name.
    pub(super) fn seed_title(&mut self, raw: &str, source_repo: Option<GithubRepo>) {
        self.title = display_peer(raw);
        self.title_raw_seed = Some(raw.into());
        self.title_edited = false;
        self.checkout_naming = CheckoutNaming {
            source: source_repo.map(CheckoutNameSource::Clone),
            ..Default::default()
        };
    }
    /// Template policy replaces Clone numbering even when the text is unchanged.
    pub(super) fn apply_template_name(
        &mut self,
        name: String,
        kind: farhelm_proto::launcher::TemplateNameKind,
    ) {
        self.title = name.clone();
        self.title_raw_seed = None;
        self.title_edited = true;
        self.checkout_naming = CheckoutNaming {
            source: Some(CheckoutNameSource::Template { base: name, kind }),
            ..Default::default()
        };
    }
    /// Retarget only occupancy search, preserving the naming source and title.
    /// The renderer owner supplies the finite system publication/key policy.
    pub(super) fn retarget_naming(&mut self, host: Option<CreateTarget>) {
        self.checkout_naming
            .retarget(host, self.destination.repo().cloned());
    }
    /// Advance the plain series after the owner admits an occupied default reply.
    /// Retained-attempt freeze remains an explicit observation of that admission.
    pub(super) fn advance_naming(&mut self, reconciling: bool) -> bool {
        self.checkout_naming.advance(reconciling)
    }

    /// Tab changes select a request variant without copying or clearing drafts.
    /// The returned fact lets each caller keep its existing retry-key rule.
    pub(super) fn choose_tab(&mut self, tab: LaunchTab) -> bool {
        let changed = self.tab != tab;
        self.tab = tab;
        changed
    }

    /// Ordinary typing claims the displayed spelling but retains its old seed.
    /// Compare literal submitted bytes: an escaped label replacing a raw seed
    /// is a real change even when the editor appears to show the same value.
    pub(super) fn edit_invocation(&mut self, text: String) -> bool {
        let changed = submitted_field(
            &self.invocation,
            self.invocation_edited,
            self.invocation_raw_seed.as_deref(),
        ) != text;
        self.invocation = text;
        self.invocation_edited = true;
        changed
    }

    /// Template text replaces source ownership, even when its spelling matches.
    pub(super) fn apply_template_invocation(&mut self, text: String) {
        self.edit_invocation(text);
        self.invocation_raw_seed = None;
    }

    /// A source's invocation is distinct from its stored command metadata.
    /// Escape only the editor display; untouched requests retain exact bytes.
    pub(super) fn seed_invocation(&mut self, raw: &str) {
        self.invocation = display_peer(raw);
        self.invocation_raw_seed = Some(raw.to_string());
        self.invocation_edited = false;
    }

    /// The user's explicit answer never infers command safety from its text.
    /// Unanswered and explicit false are distinct, even before a valid command.
    pub(super) fn choose_command_yolo(&mut self, yolo: bool) -> bool {
        let changed = self.command_yolo != Some(yolo);
        self.command_yolo = Some(yolo);
        changed
    }

    /// Clearing a declaration hides Resume without destroying its retained draft.
    /// The declaration itself is submitted metadata even with Resume disabled.
    pub(super) fn choose_command_agent(&mut self, agent: Option<LaunchHarness>) -> bool {
        let changed = self.command_agent != agent;
        self.command_agent = agent;
        changed
    }

    /// Resume visibility changes independently of its retained text ownership.
    /// Report only its effective request change; hidden retained text survives.
    pub(super) fn choose_command_resume(&mut self, enabled: bool) -> bool {
        let before = self.effective_resume();
        self.command_resume_on = enabled;
        before != self.effective_resume()
    }

    /// Ordinary resume typing claims display text without discarding its seed.
    /// A hidden editor has no request value, but still accepts its retained text.
    pub(super) fn edit_command_resume(&mut self, text: String) -> bool {
        let before = self.effective_resume();
        self.command_resume = text;
        self.command_resume_edited = true;
        before != self.effective_resume()
    }

    /// Resume has request meaning only while declared and opted in. Raw bytes
    /// remain literal; this read also keeps no-op facts aligned with projection.
    fn effective_resume(&self) -> Option<String> {
        (self.command_resume_on && self.command_agent.is_some()).then(|| {
            submitted_field(
                &self.command_resume,
                self.command_resume_edited,
                self.command_resume_raw_seed.as_deref(),
            )
        })
    }

    /// A template's explicit null disables Resume and owns the empty spelling.
    pub(super) fn apply_template_resume(&mut self, resume: Option<String>) {
        self.command_resume_on = resume.is_some();
        self.command_resume = resume.unwrap_or_default();
        self.command_resume_raw_seed = None;
        self.command_resume_edited = true;
    }

    /// Source metadata seeds Resume even when absent; a non-command source
    /// instead leaves it wholly unseeded and its YOLO assertion unanswered.
    /// Invocation is seeded separately from the source's own invocation field.
    pub(super) fn seed_command_fields(&mut self, command: Option<&CommandLaunch>) {
        match command {
            Some(command) => {
                self.command_yolo = Some(command.yolo);
                self.command_agent = command.agent;
                self.command_resume_on = command.resume.is_some();
                let raw = command.resume.as_deref().unwrap_or_default();
                self.command_resume = display_peer(raw);
                self.command_resume_raw_seed = Some(raw.to_string());
                self.command_resume_edited = false;
                self.tab = LaunchTab::Command;
            }
            None => {
                self.command_yolo = None;
                self.command_agent = None;
                self.command_resume_on = false;
                self.command_resume.clear();
                self.command_resume_raw_seed = None;
                self.command_resume_edited = false;
            }
        }
    }

    /// The ordinary prompt editor represents an empty box as absence.
    /// Compare only the supported active projection; template Some("") remains
    /// distinct from that absence and dormant prompt text still survives edits.
    pub(super) fn edit_prompt(&mut self, text: String) -> bool {
        let before = self.active_prompt();
        self.prompt = (!text.is_empty()).then_some(text);
        before != self.active_prompt()
    }

    /// The same supported-agent projection used by reusable template snapshots.
    /// Exact Some("") from a template remains distinct from ordinary absence.
    fn active_prompt(&self) -> Option<String> {
        active_prompt(
            if self.tab == LaunchTab::Agent {
                farhelm_proto::launcher::LauncherKind::Agent
            } else {
                farhelm_proto::launcher::LauncherKind::Command
            },
            self.harness,
            &self.prompt,
        )
    }

    /// Shared template application has already decided dependent prompt clearing.
    /// Preserve its result exactly, including a deliberately present empty string.
    pub(super) fn apply_template_prompt(&mut self, prompt: Option<String>) {
        self.prompt = prompt;
    }

    /// Mounted source reseeding clears the prompt but retains the conversation.
    pub(super) fn clear_prompt_for_prefill(&mut self) {
        self.prompt = None;
    }

    /// Preserve literal selector text; submit retains its ordered validation.
    /// Only the empty string means absence; spacing and escapes are real edits.
    pub(super) fn edit_conversation(&mut self, text: String) -> bool {
        let changed = self.conversation != text;
        self.conversation = text;
        changed
    }

    /// Project the active intent, before its host/destination envelope exists.
    ///
    /// Missing harness means no structured request; an unanswered YOLO assertion
    /// means no command request. Empty commands still project: submit owns their
    /// refusal and retry exemption, before the unanswered-assertion refusal.
    /// Malformed wrapper text is omitted because render-time retry matching has always
    /// been lenient; actual submission must first enforce its wrapper guard.
    /// Custom flags likewise retain the existing caller-owned validation order.
    pub(super) fn launch_intent(&self) -> Option<LaunchIntent> {
        match self.tab {
            LaunchTab::Agent => self.structured_selection().map(LaunchIntent::Structured),
            LaunchTab::Command => command_intent(
                submitted_field(
                    &self.invocation,
                    self.invocation_edited,
                    self.invocation_raw_seed.as_deref(),
                ),
                self.command_yolo,
                self.command_agent,
                self.command_resume_on,
                &submitted_field(
                    &self.command_resume,
                    self.command_resume_edited,
                    self.command_resume_raw_seed.as_deref(),
                ),
            )
            .map(LaunchIntent::Command),
        }
    }

    /// Assemble one complete binding from this draft and caller-accepted facts.
    ///
    /// This projection grants no submission authority. In particular an
    /// unselected repository query still produces a candidate, so submit keeps
    /// its existing prompt/selector/destination refusal order. Only an absent
    /// host produces None; context.launch is already selected or frozen by the
    /// caller, which need not have admitted this candidate. Wrapper, custom
    /// flags, prompt and command validation stay with the ordered caller guards.
    pub(super) fn request(&self, context: RequestContext<'_>) -> Option<IntentBinding> {
        let (host, incarnation) = context.host?;
        let title = self.effective_title();
        // Query/selected-repository candidates may lack an accepted checkout;
        // only semantic Folder intent uses ordinary optional-title identity.
        let title = if matches!(self.destination, DestinationDraft::Existing { .. }) {
            api::optional_create_title(&title)
                .unwrap_or_default()
                .to_owned()
        } else {
            title
        };
        let (cwd, github_checkout) = match context.checkout {
            None => (
                match &self.destination {
                    DestinationDraft::Existing { cwd } => cwd.clone(),
                    DestinationDraft::GithubInput { .. } | DestinationDraft::Github { .. } => {
                        self.submitted_cwd()
                    }
                },
                None,
            ),
            Some(CheckoutInput::Preview { repo, preview }) => (
                preview.cwd.raw().to_owned(),
                Some(GithubCheckoutRequest {
                    repo: repo.identifier(),
                    title: Some(title.clone()),
                    preview: preview.clone(),
                }),
            ),
            Some(CheckoutInput::Accepted { cwd, request }) => {
                (cwd.to_owned(), Some(request.clone()))
            }
        };
        let (prompt, conversation) = match &context.launch {
            LaunchIntent::Structured(selection) => (
                active_prompt(
                    farhelm_proto::launcher::LauncherKind::Agent,
                    Some(selection.harness),
                    &self.prompt,
                ),
                (!self.conversation.is_empty()).then(|| self.conversation.clone()),
            ),
            LaunchIntent::Command(_) => (None, None),
        };
        Some(IntentBinding {
            prompt,
            conversation,
            host,
            incarnation: incarnation.to_owned(),
            cwd,
            github_checkout,
            agent: context.launch,
            title,
            replace_source: context.replace_source.map(str::to_owned),
        })
    }

    /// Reuse structured normalization after the post-mint missing-agent and
    /// wrapper guards, before that caller refuses a changed active mode.
    pub(super) fn structured_selection(&self) -> Option<LaunchSelection> {
        let mut selection = self.editing_selection(self.harness?);
        if !self.is_restart() {
            selection.permissions = crate::launch_composer::normalized_permissions(
                selection.harness,
                selection.permissions,
            );
        }
        Some(selection)
    }

    /// Restart's one-time conversation is separate from reusable choices.
    /// The command projection already preserves each untouched field's bytes.
    pub(super) fn restart_edit(&self) -> Option<api::RestartEdit> {
        match self.launch_intent()? {
            LaunchIntent::Structured(selection) if self.conversation.is_empty() => {
                Some(api::RestartEdit::Agent(selection))
            }
            LaunchIntent::Structured(selection) => Some(api::RestartEdit::NamedAgent(
                selection,
                self.conversation.clone(),
            )),
            LaunchIntent::Command(command) => Some(api::RestartEdit::Command(command)),
        }
    }

    /// A Guarded answer must still make an actual edit to the stored launch.
    pub(super) fn restart_guarded_changes(&self) -> bool {
        let mut guarded = self.clone();
        guarded.permissions = Some(LaunchPermission::Guarded);
        guarded.permission_flags.clear();
        guarded
            .restart_baseline
            .as_ref()
            .is_none_or(|baseline| guarded.restart_edit().as_ref() != Some(baseline))
    }

    /// Refuse held changes after validation removes matching assignments.
    /// Initial prompts and clearing a command's resume choice have no matching
    /// case. This runs before the shared fold or any semantic publication;
    /// search results keep their separate presence-based refusal.
    pub(super) fn restart_template_refusal(
        &self,
        template: &farhelm_proto::launcher::LaunchTemplate,
    ) -> Option<String> {
        if !self.is_restart() {
            return None;
        }
        let f = &template.fields;
        let field = if f.kind.is_some() {
            Some("launch kind")
        } else if f.agent.is_some() {
            Some("agent type")
        } else if f.host.is_some() {
            Some("host")
        } else if f.destination.is_some() {
            Some("destination")
        } else if f.name.is_some() {
            Some("session name")
        } else if f.prompt.is_some() {
            Some("initial prompt")
        } else if self.tab == LaunchTab::Command && f.resume_command == Some(None) {
            Some("resume choice")
        } else {
            None
        };
        field.map(|field| {
            farhelm_proto::launcher::TemplateRefusal {
                template: template.name.clone(),
                field,
                reason: format!("cannot change here: this dialog keeps {field} fixed for restart"),
            }
            .to_string()
        })
    }

    /// Compatibility operates on the raw choice, before request defaults apply.
    /// A target fallback permits the first harness choice to reuse the same
    /// reconciliation rule as a later switch without inventing a prior agent.
    pub(super) fn editing_selection(&self, fallback: LaunchHarness) -> LaunchSelection {
        LaunchSelection {
            harness: self.harness.unwrap_or(fallback),
            model: self.model.clone(),
            effort: self.effort,
            permissions: self.permissions,
            permission_flags: crate::launch_composer::permission_words(
                self.permissions,
                &self.permission_flags,
            ),
            wrapper_prefix: super::wrapper_words(&self.wrapper).ok().flatten(),
            workspace_trust: None,
        }
    }

    /// Reconciliation can touch the whole agent group. Compare its existing
    /// submitted projection only when its affected editors parse successfully;
    /// render-time leniency must not prove malformed words equal to absence.
    fn retry_selection(&self) -> Option<LaunchSelection> {
        super::wrapper_words(&self.wrapper).ok()?;
        self.retry_permissions()?;
        self.structured_selection()
    }

    /// Effective permission and Custom words share one request identity. Raw
    /// omission and an explicit default can differ without changing that pair.
    fn retry_permissions(&self) -> Option<(Option<LaunchPermission>, Option<Vec<String>>)> {
        let permission = if self.is_restart() {
            self.permissions
        } else {
            crate::launch_composer::normalized_permissions(self.harness?, self.permissions)
        };
        let words = if self.permissions == Some(LaunchPermission::Custom) {
            Some(farhelm_proto::split_permission_flags(&self.permission_flags).ok()?)
        } else {
            None
        };
        Some((permission, words))
    }

    /// Model choices compare accepted request bytes, never an escaped editor.
    /// Only model and effort can change on the same-harness control paths.
    fn model_and_effort(&self) -> (Option<String>, Option<LaunchEffort>) {
        // editing_selection submits this accepted raw value directly; model()
        // consumers may separately escape it for display. Do not parse unrelated
        // wrapper/permission editors to decide a model-only change.
        (self.model.clone(), self.effort)
    }

    /// Choose an agent using the existing catalog-aware compatibility policy.
    /// A genuine owner switch loads that agent's remembered wrapper. Repeated
    /// choices retain raw flags rather than round-tripping through shell words.
    pub(super) fn choose_harness(
        &mut self,
        harness: LaunchHarness,
        catalog: Option<&[api::LaunchCatalogModel]>,
        remembered_wrapper: String,
        origin: ChoiceOrigin,
    ) -> AgentChoiceOutcome {
        let submitted_before = self.retry_selection();
        let old_harness = self.harness;
        let mut before = self.editing_selection(harness);
        if origin == ChoiceOrigin::Control {
            before.wrapper_prefix = None;
        }
        let (selection, owner) = crate::launch_composer::reconcile_harness_for_catalog_read(
            before.clone(),
            self.model_owner,
            harness,
            catalog,
        );
        let reset_reason =
            super::draft_reconciliation_reason(&before, &selection, self.permissions_explicit);
        let replace_flags = match origin {
            ChoiceOrigin::Control => old_harness != Some(harness),
            ChoiceOrigin::Search => {
                selection.harness != before.harness || selection.permissions != before.permissions
            }
        };
        self.accept_reconciled(&selection, owner, false);
        if old_harness != Some(selection.harness) {
            self.wrapper = remembered_wrapper;
            self.wrapper_explicit = false;
        }
        if replace_flags {
            self.permission_flags = joined(selection.permission_flags.as_ref());
        }
        AgentChoiceOutcome {
            reset_reason,
            selection_changed: submitted_before.is_none()
                || submitted_before != self.retry_selection(),
        }
    }

    /// Accept a catalog model without changing the existing path's semantics.
    /// Control picks within one harness clear unsupported effort with the
    /// model-specific notice. Search always runs whole-harness reconciliation.
    pub(super) fn choose_catalog_model(
        &mut self,
        id: String,
        harness: LaunchHarness,
        catalog: Option<&[api::LaunchCatalogModel]>,
        remembered_wrapper: String,
        origin: ChoiceOrigin,
    ) -> AgentChoiceOutcome {
        let old_harness = self.harness;
        if origin == ChoiceOrigin::Control && old_harness == Some(harness) {
            let model_before = self.model_and_effort();
            let offered = catalog
                .unwrap_or_default()
                .iter()
                .find(|model| model.id == id && model.harness == harness)
                .map(|model| model.efforts.as_slice())
                .unwrap_or_default();
            let reset_reason = self.effort.filter(|effort| !offered.contains(effort)).map(|_| {
                self.effort = None;
                "The selected effort is not in Farhelm's offering for that model, so it was cleared".to_string()
            });
            self.model = Some(id);
            self.model_raw_seed = None;
            self.model_edited = true;
            self.model_owner = None;
            return AgentChoiceOutcome {
                reset_reason,
                selection_changed: model_before != self.model_and_effort(),
            };
        }
        let submitted_before = self.retry_selection();
        let mut before = self.editing_selection(harness);
        if origin == ChoiceOrigin::Control {
            before.wrapper_prefix = None;
        }
        let mut proposed = before.clone();
        proposed.model = Some(id);
        let (selection, owner) = crate::launch_composer::reconcile_harness_for_catalog_read(
            proposed,
            (origin == ChoiceOrigin::Search).then_some(harness),
            harness,
            catalog,
        );
        let reset_reason =
            super::draft_reconciliation_reason(&before, &selection, self.permissions_explicit);
        let replace_flags = origin == ChoiceOrigin::Control
            || selection.harness != before.harness
            || selection.permissions != before.permissions;
        self.accept_reconciled(&selection, owner, origin == ChoiceOrigin::Control);
        if old_harness != Some(selection.harness) {
            self.wrapper = remembered_wrapper;
            self.wrapper_explicit = false;
        }
        if replace_flags {
            self.permission_flags = joined(selection.permission_flags.as_ref());
        }
        AgentChoiceOutcome {
            reset_reason,
            selection_changed: submitted_before.is_none()
                || submitted_before != self.retry_selection(),
        }
    }

    /// Accept a typed custom ID without treating unknown catalog as rejection.
    /// Typing never switches the selected harness; the control has already
    /// refused IDs known to belong elsewhere before this transition is called.
    pub(super) fn choose_custom_model(
        &mut self,
        id: String,
        harness: LaunchHarness,
        catalog: Option<&[api::LaunchCatalogModel]>,
    ) -> AgentChoiceOutcome {
        let model_before = self.model_and_effort();
        let mut before = self.editing_selection(harness);
        before.harness = harness;
        before.wrapper_prefix = None;
        let mut selection = before.clone();
        selection.model = Some(id);
        let reason = if let Some(effort) = selection.effort
            && let Some(catalog) = catalog
            && !crate::launch_composer::compatible_efforts(
                harness,
                selection.model.as_deref(),
                catalog,
            )
            .contains(&effort)
        {
            self.effort = None;
            selection.effort = None;
            crate::launch_composer::reconciliation_reset_reason(&before, &selection)
        } else {
            None
        };
        self.model = selection.model;
        self.model_raw_seed = None;
        self.model_edited = true;
        self.model_owner = Some(harness);
        AgentChoiceOutcome {
            reset_reason: reason,
            selection_changed: model_before != self.model_and_effort(),
        }
    }

    /// Choosing the default model does not erase an independently chosen effort.
    pub(super) fn choose_default_model(&mut self) -> bool {
        let changed = self.model_and_effort().0.is_some();
        self.model = None;
        self.model_raw_seed = None;
        self.model_edited = false;
        self.model_owner = None;
        changed
    }

    /// Report a real effort change so repeated choices can remain retries.
    pub(super) fn choose_effort(&mut self, effort: Option<LaunchEffort>) -> bool {
        let changed = self.effort != effort;
        self.effort = effort;
        changed
    }

    /// A deliberate mode choice owns permissions and clears unrelated flags.
    /// Custom mode retains its raw buffer so returning to its editor loses no text.
    /// Its fact compares effective defaults and valid Custom words, so an
    /// explicit default can be unchanged while malformed words remain conservative.
    pub(super) fn choose_permissions(&mut self, permission: Option<LaunchPermission>) -> bool {
        let before = self.retry_permissions();
        self.permissions = permission;
        if permission != Some(LaunchPermission::Custom) {
            self.permission_flags.clear();
        }
        self.permissions_explicit = true;
        before.is_none() || before != self.retry_permissions()
    }

    /// Keep the exact editor buffer even when it is not yet a valid argument list.
    /// Only two successful parses can prove an unchanged word vector; invalid
    /// Custom input retains conservative retirement while it remains repairable.
    pub(super) fn edit_permission_flags(&mut self, text: String) -> bool {
        let before = farhelm_proto::split_permission_flags(&self.permission_flags);
        let after = farhelm_proto::split_permission_flags(&text);
        let changed = !matches!((before, after), (Ok(before), Ok(after)) if before == after);
        self.permission_flags = text;
        self.permissions_explicit = true;
        changed
    }

    /// Editing a remembered prefix makes it an explicit reusable choice.
    /// Valid quoting/spacing changes preserve retry identity when literal words
    /// match; malformed nonempty text never compares equal to an absent prefix.
    pub(super) fn edit_wrapper(&mut self, text: String) -> bool {
        let changed = !matches!((super::wrapper_words(&self.wrapper), super::wrapper_words(&text)), (Ok(before), Ok(after)) if before == after);
        self.wrapper = text;
        self.wrapper_explicit = true;
        changed
    }

    /// Reapply New's passive permission seed while preserving launch context.
    /// Clearing the selected agent also ends its ownership of custom words;
    /// leaving them dormant would let search restore them under another CLI.
    /// Command, tab, one-time inputs and placement remain exactly as they were.
    pub(super) fn reset_choices(&mut self, permission: Option<LaunchPermission>) {
        self.harness = None;
        self.choose_default_model();
        self.effort = None;
        self.permission_flags.clear();
        self.permissions = permission;
        self.permissions_explicit = false;
        self.wrapper.clear();
        self.wrapper_explicit = false;
    }

    /// A stored launch replaces only the structured group and is explicit.
    /// The caller supplies model-owner evidence: Clone has source ownership;
    /// a recent uses the current catalog's custom_model_owner decision. The
    /// returned fact concerns normalized submitted choices, not these ownership
    /// writes or the caller's separate destination/tab admission and UI work.
    pub(super) fn restore_launch(
        &mut self,
        selection: &LaunchSelection,
        owner: Option<LaunchHarness>,
    ) -> bool {
        let before = self.retry_selection();
        self.accept_reconciled(selection, owner, false);
        self.permissions = crate::launch_composer::normalized_permissions(
            selection.harness,
            selection.permissions,
        );
        self.permission_flags = joined(selection.permission_flags.as_ref());
        self.wrapper = joined(selection.wrapper_prefix.as_ref());
        self.permissions_explicit = true;
        self.wrapper_explicit = true;
        before.is_none() || before != self.retry_selection()
    }

    /// A command/legacy prefill has no hidden structured choice to inherit.
    /// Clear only that group: the caller has already seeded its new command,
    /// and mounted source changes preserve conversation, placement and naming.
    pub(super) fn clear_for_command_seed(&mut self) {
        self.reset_choices(None);
        self.permissions_explicit = true;
    }

    /// A template owns its copied permission words, including an explicit default.
    pub(super) fn apply_template_permissions(
        &mut self,
        permission: Option<LaunchPermission>,
        flags: String,
    ) {
        // An explicit template reset requests today's default. Unlike an
        // absent template field, it must not restore a historical no-flag
        // baseline and appear changed when the helm would compile Guarded.
        self.permissions = if self.is_restart() && permission.is_none() {
            self.harness
                .and_then(|harness| harness.request_permission(None))
        } else {
            permission
        };
        self.permission_flags = flags;
        self.permissions_explicit = true;
    }

    /// Undo restores accepted values and ownership together, never an attempt key.
    pub(super) fn restore(&mut self, before: Self) {
        *self = before;
    }

    /// Shared reconciliation assignment keeps raw model ownership with its value.
    fn accept_reconciled(
        &mut self,
        selection: &LaunchSelection,
        owner: Option<LaunchHarness>,
        edited: bool,
    ) {
        self.harness = Some(selection.harness);
        self.model = selection.model.clone();
        self.model_raw_seed = (!edited).then(|| selection.model.clone()).flatten();
        self.model_edited = edited;
        self.model_owner = owner;
        self.effort = selection.effort;
        self.permissions = selection.permissions;
    }
}

/// Stored literal words become an initial editor buffer only at ownership changes.
fn joined(words: Option<&Vec<String>>) -> String {
    words.map(shell_words::join).unwrap_or_default()
}

/// Read-only per-field subscriptions retain the original effect boundaries.
/// Memo peeks can be stale in one event turn; imperative callers use snapshot().
#[derive(Clone, Copy)]
pub(super) struct DraftFields {
    pub(super) cwd: Memo<String>,
    pub(super) cwd_edited: Memo<bool>,
    pub(super) title: Memo<String>,
    pub(super) title_raw_seed: Memo<Option<String>>,
    pub(super) title_edited: Memo<bool>,
    pub(super) destination_draft: Memo<DestinationDraft>,
    pub(super) clone_checkout_naming: Memo<CheckoutNaming>,
    pub(super) launch_tab: Memo<LaunchTab>,
    pub(super) invocation: Memo<String>,
    pub(super) invocation_edited: Memo<bool>,
    pub(super) command_yolo: Memo<Option<bool>>,
    pub(super) command_agent: Memo<Option<LaunchHarness>>,
    pub(super) command_resume_on: Memo<bool>,
    pub(super) command_resume: Memo<String>,
    pub(super) command_resume_edited: Memo<bool>,
    pub(super) prompt: Memo<Option<String>>,
    pub(super) conversation: Memo<String>,
    pub(super) structured_harness: Memo<Option<LaunchHarness>>,
    pub(super) structured_model: Memo<Option<String>>,
    pub(super) structured_model_raw_seed: Memo<Option<String>>,
    pub(super) structured_model_edited: Memo<bool>,
    pub(super) structured_effort: Memo<Option<LaunchEffort>>,
    pub(super) structured_permissions: Memo<Option<LaunchPermission>>,
    pub(super) structured_permissions_is_explicit: Memo<bool>,
    pub(super) structured_permission_flags: Memo<String>,
    pub(super) structured_wrapper: Memo<String>,
    pub(super) wrapper_is_explicit: Memo<bool>,
}

/// Existing picker handles observed only after a scalar edit earns admission.
/// History promotion deliberately keeps its tracked target/fetched reads: an
/// opening effect that invokes a choice must retain those dependencies. These
/// are aliases to UI/history state, not another copy of accepted launch fields.
#[derive(Clone, Copy)]
pub(super) struct ChoiceTargets {
    pub(super) partial_template_hint: Signal<bool>,
    pub(super) offered_history: Signal<Option<(CreateTarget, api::LaunchHistory)>>,
    pub(super) create_target: Signal<Option<CreateTarget>>,
    pub(super) fetched_history: Signal<Option<(CreateTarget, api::LaunchHistory)>>,
}

impl ChoiceTargets {
    /// A deliberate choice may reveal refreshed history for its current install.
    /// Keep the shared helper's tracked reads inside the admitted interaction.
    fn promote_history(self) {
        super::promote_fetched_history_snapshot(
            self.offered_history,
            self.create_target,
            self.fetched_history,
        );
    }
}

/// Existing source-opening handles retain their separate lifetimes and reads.
/// A source generation replaces launch fields once; host binding keeps resolving
/// later without reseeding. Nothing in this bundle caches a draft or host target.
#[derive(Clone, Copy)]
pub(super) struct SourceTargets {
    pub(super) create_target: Signal<Option<CreateTarget>>,
    pub(super) bound_target: Signal<Option<CreateTarget>>,
    pub(super) remembered_destination: Signal<Option<CreateTarget>>,
    pub(super) prefill_applied: Signal<Option<u64>>,
    pub(super) mount_seeded: Signal<bool>,
    pub(super) partial_template_hint: Signal<bool>,
    pub(super) auto_launch_pending: Signal<bool>,
    pub(super) quick_launch_hidden: Signal<bool>,
    pub(super) pending_initial_action: Signal<Option<crate::launch_composer::ComposerSearchResult>>,
    pub(super) pending_initial_templates:
        Signal<std::collections::VecDeque<farhelm_proto::launcher::LaunchTemplate>>,
    pub(super) clone_host_state: Signal<CloneHostState>,
    pub(super) preview_revision: Signal<u64>,
    pub(super) browse_generation: Signal<u64>,
    pub(super) browse_request: Signal<Option<BrowseAuthority>>,
    pub(super) browse_result: Signal<Option<(BrowseAuthority, api::DirectoryBrowse)>>,
    pub(super) browse_error: Signal<Option<String>>,
    pub(super) error: Signal<Option<String>>,
    pub(super) preparation_error: Signal<Option<String>>,
    pub(super) composer_reset_reason: Signal<Option<String>>,
    pub(super) permission_escalation: Signal<Option<PermissionEscalation>>,
    pub(super) template_panel: Signal<Option<(super::super::save_template::Candidate, String)>>,
}

/// Placement edits share the existing provenance and observation handles.
/// These are aliases, not retained authority: admission reads the live claims
/// before a captured history or browse result can replace any draft fields.
#[derive(Clone, Copy)]
pub(super) struct PlacementTargets {
    pub(super) choices: ChoiceTargets,
    pub(super) live_destination: Signal<Option<CreateTarget>>,
    pub(super) remembered_destination: Signal<Option<CreateTarget>>,
    pub(super) clone_host_state: Signal<CloneHostState>,
    pub(super) history_activation_attempts: Signal<u64>,
    pub(super) preview_revision: Signal<u64>,
    pub(super) browse_generation: Signal<u64>,
    pub(super) browse_request: Signal<Option<BrowseAuthority>>,
    pub(super) browse_result: Signal<Option<(BrowseAuthority, api::DirectoryBrowse)>>,
    pub(super) browse_error: Signal<Option<String>>,
}

impl PlacementTargets {
    /// Capture invalid provenance before adoption or withdrawal can erase it.
    /// Equal bytes still end the old pair when they repair an invalid claim.
    fn correcting_invalid(self) -> bool {
        !remembered_destination_matches(
            self.remembered_destination.peek().as_ref(),
            self.live_destination.peek().as_ref(),
        )
    }

    /// Deliberate placement revokes generation as well as path evidence, even
    /// when an equal choice keeps its retry identity.
    fn invalidate_browse(self) {
        invalidate_directory_browse(
            self.browse_generation,
            self.browse_request,
            self.browse_result,
            self.browse_error,
        );
    }
}

/// Repository completion keeps search and suggestion UI separate from the draft.
#[derive(Clone, Copy)]
pub(super) struct RepositoryTargets {
    pub(super) open: Signal<bool>,
    pub(super) index: Signal<usize>,
    pub(super) search: Signal<String>,
    pub(super) search_open: Signal<bool>,
}

/// Folder typing and accepted choices have different open/close completion.
#[derive(Clone, Copy)]
pub(super) struct FolderTargets {
    pub(super) open: Signal<bool>,
    pub(super) active: Signal<Option<usize>>,
}

/// Recovery stores values only; it cannot execute an edit or install itself.
/// Authority, caches, epochs and retained attempts deliberately stay external.
#[derive(Clone)]
pub(super) struct PermissionUndo {
    draft: LaunchDraft,
    composer_reset_reason: Option<String>,
    chosen_host: Option<HostId>,
    clone_host_state: CloneHostState,
    remembered_destination: Option<CreateTarget>,
    model_draft: String,
    model_draft_error: Option<String>,
}

/// The existing external edit context captured at each actual switch boundary.
#[derive(Clone, Copy)]
pub(super) struct RecoveryTargets {
    pub(super) composer_reset_reason: Signal<Option<String>>,
    pub(super) clone_host_state: Signal<CloneHostState>,
    pub(super) remembered_destination: Signal<Option<CreateTarget>>,
    pub(super) model_draft: Signal<String>,
    pub(super) model_draft_error: Signal<Option<String>>,
}

/// Default/custom cleanup may complete inside its concrete operation. Catalog
/// cleanup stays in the renderer after its completed permission observer, so
/// passing this bundle must not move that capture/completion boundary.
#[derive(Clone, Copy)]
pub(super) struct ModelTargets {
    pub(super) reset_reason: Signal<Option<String>>,
    pub(super) draft: Signal<String>,
    pub(super) error: Signal<Option<String>>,
    pub(super) open: Signal<bool>,
    pub(super) active: Signal<Option<usize>>,
}

impl ModelTargets {
    /// Equal admitted choices still discard the pending picker candidate.
    fn complete(mut self) {
        self.error.set(None);
        self.draft.set(String::new());
        self.open.set(false);
        self.active.set(None);
    }
}

/// Immutable existing facts for one acceptance, never another authority.
/// Preferences remain a handle for the original application-time peek.
pub(super) struct SearchInputs<'a> {
    pub(super) templates: &'a [farhelm_proto::launcher::LaunchTemplate],
    pub(super) hosts: &'a [HostOption],
    pub(super) catalog: Option<&'a [api::LaunchCatalogModel]>,
    pub(super) host_fixed: bool,
    pub(super) history_target: Option<CreateTarget>,
    pub(super) preferences: SharedPreferences,
}

/// Search shares existing placement and immutable recovery aliases.
#[derive(Clone, Copy)]
pub(super) struct SearchTargets {
    pub(super) placement: PlacementTargets,
    pub(super) recovery: RecoveryTargets,
    pub(super) template_error: Signal<Option<String>>,
}

/// Returned after publication/key settlement, before renderer completion.
pub(super) struct SearchAcceptance {
    pub(super) before: PermissionSwitchBefore,
    pub(super) automatic_permission: bool,
    pub(super) browse_path: Option<String>,
    pub(super) is_template: bool,
}

/// Only concrete owner operations choose a rule; callers cannot bypass the gate.
/// Source reconciliation and checkout naming are observations, allowed while a
/// submit is busy, with different publication and ordinary-key consequences.
#[derive(Clone, Copy)]
enum EditRule {
    Ordinary,
    Template,
    SourceReconciliation,
    CheckoutNaming,
}

/// Publication is independent of key retirement for system observations.
/// Target-only reconciliation retires without rewriting the draft; naming
/// publishes only an actual sequence change and never retires ordinary authority.
struct EditFacts {
    publish: bool,
    changed: bool,
}

/// The only mounted draft publisher, sharing the form's original live handles.
/// Callers can observe fields or edit owned snapshots, but cannot install one.
/// Concrete operations choose gate/key rules internally; submission still owns
/// mint/reply lifetime through the same external retry slot.
#[derive(Clone, Copy)]
pub(super) struct LaunchDraftState {
    value: Signal<LaunchDraft>,
    fields: DraftFields,
    ops: OpLock,
    intent_key: Signal<Option<(String, IntentBinding)>>,
    chosen_host: Signal<Option<HostId>>,
}

impl LaunchDraftState {
    /// Current values without adding a whole-draft reactive subscription.
    pub(super) fn snapshot(self) -> LaunchDraft {
        self.value.peek().clone()
    }
    /// Components may subscribe to fields but cannot obtain writable aliases.
    pub(super) fn fields(self) -> DraftFields {
        self.fields
    }

    /// Keep all converted publication and edit-induced retirement in one place.
    /// Ordinary/template admission precedes every read/preamble inside this
    /// wrapper; finite system rules remain available while busy. A local copy
    /// permits conditional Memo observations without a live write borrow.
    /// Template/name rules preserve the complete pair; domain refusals publish
    /// nothing, while equal accepted assignments can still claim raw ownership.
    fn apply_edit<R>(
        mut self,
        rule: EditRule,
        apply: impl FnOnce(&mut LaunchDraft) -> (R, EditFacts),
    ) -> Option<R> {
        if matches!(rule, EditRule::Ordinary | EditRule::Template) && self.ops.busy_now() {
            return None;
        }
        let mut current = self.snapshot();
        let (result, facts) = apply(&mut current);
        if facts.publish {
            self.value.set(current);
        }
        if facts.changed && !matches!(rule, EditRule::CheckoutNaming | EditRule::Template) {
            self.intent_key.set(None);
        }
        Some(result)
    }

    /// Ordinary equality still publishes raw ownership and accepted buffers.
    /// Return admission, so unchanged choices retain their UI completion contract.
    fn apply_ordinary(self, apply: impl FnOnce(&mut LaunchDraft) -> bool) -> bool {
        self.apply_edit(EditRule::Ordinary, |draft| {
            let changed = apply(draft);
            (
                (),
                EditFacts {
                    publish: true,
                    changed,
                },
            )
        })
        .is_some()
    }

    /// Ordinary name ownership follows the existing folder/checkout name policy.
    /// Admission is returned independently of the plain transition's key fact.
    pub(super) fn edit_title(self, text: String) -> bool {
        self.apply_ordinary(|draft| draft.edit_title(text))
    }

    /// Literal command edits retire authority only when submitted bytes change.
    /// Equality still claims editor ownership; a busy edit claims no text at all.
    pub(super) fn edit_invocation(self, text: String) -> bool {
        self.apply_ordinary(|draft| draft.edit_invocation(text))
    }

    /// An explicit assertion remains distinct from the unanswered command state.
    /// Reapplying its current answer is still an admitted interaction.
    pub(super) fn choose_command_yolo(self, yolo: bool) -> bool {
        self.apply_ordinary(|draft| draft.choose_command_yolo(yolo))
    }

    /// Declaration edits preserve dormant Resume text and its opt-in choice.
    /// Only the existing submitted metadata fact decides ordinary retirement.
    pub(super) fn choose_command_agent(self, agent: Option<LaunchHarness>) -> bool {
        self.apply_ordinary(|draft| draft.choose_command_agent(agent))
    }

    /// Resume visibility may change without changing the effective command.
    /// Apply that retained state even when the plain request fact is unchanged.
    pub(super) fn choose_command_resume(self, enabled: bool) -> bool {
        self.apply_ordinary(|draft| draft.choose_command_resume(enabled))
    }

    /// Literal Resume ownership and hidden retention use the same live gate.
    /// The plain transition keeps effective Resume separate from display text.
    pub(super) fn edit_command_resume(self, text: String) -> bool {
        self.apply_ordinary(|draft| draft.edit_command_resume(text))
    }

    /// UI empty prompt means absence; template empty prompts have a separate path.
    /// Dormant prompt assignments still occur when their request fact is equal.
    pub(super) fn edit_prompt(self, text: String) -> bool {
        self.apply_ordinary(|draft| draft.edit_prompt(text))
    }

    /// Conversation edits retain exact selector bytes and current refusal rules.
    /// This ordinary edit does not acquire submission or confirmation authority.
    pub(super) fn edit_conversation(self, text: String) -> bool {
        self.apply_ordinary(|draft| draft.edit_conversation(text))
    }

    /// Keep unfinished wrapper text editable and compare only proven word equality.
    /// Admission preserves raw explicitness even when quoting changes no words.
    pub(super) fn edit_wrapper(self, text: String) -> bool {
        self.apply_ordinary(|draft| draft.edit_wrapper(text))
    }

    /// Custom flags share every ordinary editor's live queued-event protection.
    /// The existing parser fact remains conservative for malformed drafts.
    pub(super) fn edit_permission_flags(self, text: String) -> bool {
        self.apply_ordinary(|draft| draft.edit_permission_flags(text))
    }

    /// Effort choice promotes history only after live admission, even on equality.
    /// Its preamble order matches the segmented control's existing interaction.
    pub(super) fn choose_effort(
        self,
        effort: Option<LaunchEffort>,
        mut targets: ChoiceTargets,
    ) -> bool {
        self.apply_ordinary(|draft| {
            targets.partial_template_hint.set(false);
            targets.promote_history();
            draft.choose_effort(effort)
        })
    }

    /// Controls and guarded answers share one permission edit and retry decision.
    /// Confirmation binding/take and subsequent submit remain with the caller.
    pub(super) fn choose_permissions(
        self,
        permission: Option<LaunchPermission>,
        mut targets: ChoiceTargets,
    ) -> bool {
        self.apply_ordinary(|draft| {
            targets.partial_template_hint.set(false);
            targets.promote_history();
            draft.choose_permissions(permission)
        })
    }

    /// Click/Enter tab choices dismiss the hint before promoting history.
    /// The admitted return lets the renderer retain its search-focus completion.
    pub(super) fn choose_launch_tab(self, tab: LaunchTab, mut targets: ChoiceTargets) -> bool {
        self.apply_ordinary(|draft| {
            targets.partial_template_hint.set(false);
            targets.promote_history();
            draft.choose_tab(tab)
        })
    }

    /// Arrow/Home/End navigation retains its distinct hint order and tab focus.
    /// It shares the plain tab transition and ordinary key rule with click/Enter.
    pub(super) fn navigate_launch_tab(self, tab: LaunchTab, mut targets: ChoiceTargets) -> bool {
        self.apply_ordinary(|draft| {
            targets.promote_history();
            let changed = draft.choose_tab(tab);
            targets.partial_template_hint.set(false);
            changed
        })
    }

    // Placement composes the existing live fences with pure assignments. Keep
    // history reads inside admission and observation-only browse work outside.

    /// Explicit host selection revokes historical authority even for the same id.
    /// Publish no draft: host choice uses the parent's original shared handle,
    /// and queued history callbacks must see its synchronous destination fence.
    pub(super) fn choose_host(
        mut self,
        id: HostId,
        hosts: &[HostOption],
        mut targets: PlacementTargets,
    ) -> bool {
        self.apply_edit(EditRule::Ordinary, |_| {
            targets.choices.partial_template_hint.set(false);
            self.chosen_host.set(Some(id));
            targets
                .live_destination
                .set(history_target(hosts, Some(id)));
            targets.remembered_destination.set(None);
            targets.invalidate_browse();
            targets.clone_host_state.set(CloneHostState::UserTookOver);
            (
                (),
                EditFacts {
                    publish: false,
                    changed: true,
                },
            )
        })
        .is_some()
    }

    /// Seed a host's default folder (SPEC.md, "Working directory") into a draft
    /// whose folder is still a default: a folder or repository the person chose
    /// in this draft is theirs, so it is left alone. Returns whether the draft
    /// took the default, which is true even when the folder already matched.
    pub(super) fn seed_default_folder(self, raw: &str) -> bool {
        self.apply_edit(EditRule::SourceReconciliation, |draft| {
            let accepted = !draft.folder_is_explicit()
                && matches!(draft.destination(), DestinationDraft::Existing { .. });
            let changed = accepted && draft.submitted_cwd() != raw;
            if changed {
                draft.seed_folder(raw);
            }
            (
                accepted,
                EditFacts {
                    publish: changed,
                    changed,
                },
            )
        })
        .unwrap_or(false)
    }

    /// Repository reselection explicitly requests fresh preview/name authority.
    /// History promotion remains tracked and admitted; even the same repository
    /// increments revision, closes its discovery search and retires the pair.
    pub(super) fn select_repository(
        self,
        repo: GithubRepo,
        mut targets: PlacementTargets,
        mut ui: RepositoryTargets,
    ) -> bool {
        self.apply_edit(EditRule::Ordinary, |draft| {
            targets.choices.partial_template_hint.set(false);
            targets.choices.promote_history();
            targets.invalidate_browse();
            targets.remembered_destination.set(None);
            ui.search.set(String::new());
            ui.search_open.set(false);
            draft.select_repository(repo);
            targets.preview_revision.with_mut(|revision| {
                *revision = revision.checked_add(1).expect("preview revision exhausted")
            });
            ui.open.set(false);
            (
                (),
                EditFacts {
                    publish: true,
                    changed: true,
                },
            )
        })
        .is_some()
    }

    /// Same-mode selection refuses before any provenance, history or UI work.
    /// A real mode change restores the dormant folder's existing raw ownership.
    pub(super) fn choose_destination_type(
        self,
        managed: bool,
        mut targets: PlacementTargets,
        mut repository_open: Signal<bool>,
    ) -> bool {
        self.apply_edit(EditRule::Ordinary, |draft| {
            if managed == draft.destination().is_managed() {
                return (
                    false,
                    EditFacts {
                        publish: false,
                        changed: false,
                    },
                );
            }
            targets.choices.partial_template_hint.set(false);
            targets.remembered_destination.set(None);
            targets.choices.promote_history();
            targets.invalidate_browse();
            repository_open.set(false);
            draft.choose_destination_type(managed);
            (
                true,
                EditFacts {
                    publish: true,
                    changed: true,
                },
            )
        })
        .unwrap_or(false)
    }

    /// The rendered read-only restriction remains the captured interaction fact.
    /// Refusal earns no ancillary work; accepted query text revokes selection
    /// authority rather than treating a proposal as an executable destination.
    pub(super) fn edit_repository_query(
        self,
        text: String,
        captured_read_only: bool,
        mut targets: PlacementTargets,
        mut ui: RepositoryTargets,
    ) -> bool {
        self.apply_edit(EditRule::Ordinary, |draft| {
            if captured_read_only {
                return (
                    false,
                    EditFacts {
                        publish: false,
                        changed: false,
                    },
                );
            }
            targets.remembered_destination.set(None);
            targets.invalidate_browse();
            draft.edit_repository_query(text);
            ui.index.set(0);
            ui.open.set(true);
            (
                true,
                EditFacts {
                    publish: true,
                    changed: true,
                },
            )
        })
        .unwrap_or(false)
    }

    /// Clearing inherited unsafe identity closes suggestions instead of typing.
    /// This is a fixed explicit operation, independent of the input's read-only
    /// restriction, and ends the old pair even when the query was already empty.
    pub(super) fn clear_repository(
        self,
        mut targets: PlacementTargets,
        mut ui: RepositoryTargets,
    ) -> bool {
        self.apply_edit(EditRule::Ordinary, |draft| {
            targets.remembered_destination.set(None);
            targets.invalidate_browse();
            draft.edit_repository_query(String::new());
            ui.index.set(0);
            ui.open.set(false);
            (
                (),
                EditFacts {
                    publish: true,
                    changed: true,
                },
            )
        })
        .is_some()
    }

    /// Equal literal paths still take editor ownership and complete suggestions.
    /// Capture invalid inherited provenance before withdrawing it; repairing that
    /// claim retires a pair even when the submitted path itself is unchanged.
    pub(super) fn edit_folder(
        self,
        text: String,
        mut targets: PlacementTargets,
        mut ui: FolderTargets,
    ) -> bool {
        self.apply_edit(EditRule::Ordinary, |draft| {
            if draft.destination().is_managed() {
                return (
                    false,
                    EditFacts {
                        publish: false,
                        changed: false,
                    },
                );
            }
            targets.choices.promote_history();
            let correcting_invalid = targets.correcting_invalid();
            let changed = draft.edit_folder(text);
            ui.active.set(None);
            ui.open.set(true);
            targets.remembered_destination.set(None);
            targets.invalidate_browse();
            (
                true,
                EditFacts {
                    publish: true,
                    changed: changed || correcting_invalid,
                },
            )
        })
        .unwrap_or(false)
    }

    /// Home is an ordinary folder choice; LocalHome also explicitly pins a host.
    /// A Recent must prove its captured installation before copying its path.
    /// Equal admitted paths still promote history and close the picker.
    pub(super) fn choose_folder(
        mut self,
        choice: FolderChoice,
        local_host: Option<HostId>,
        local_destination: Option<CreateTarget>,
        mut targets: PlacementTargets,
        mut ui: FolderTargets,
    ) -> bool {
        let accepted = self
            .apply_edit(EditRule::Ordinary, |draft| {
                if draft.destination().is_managed() {
                    return (
                        false,
                        EditFacts {
                            publish: false,
                            changed: false,
                        },
                    );
                }
                targets.choices.partial_template_hint.set(false);
                let correcting_invalid = targets.correcting_invalid();
                let host_action = matches!(&choice, FolderChoice::LocalHome);
                let path = match choice {
                    FolderChoice::Recent { cwd, target } => {
                        if !admit_history_destination(
                            target,
                            targets.live_destination,
                            targets.remembered_destination,
                            targets.history_activation_attempts,
                        ) {
                            return (
                                false,
                                EditFacts {
                                    publish: false,
                                    changed: false,
                                },
                            );
                        }
                        cwd
                    }
                    FolderChoice::Home => {
                        targets.remembered_destination.set(None);
                        "~".to_string()
                    }
                    FolderChoice::LocalHome => {
                        self.chosen_host.set(local_host);
                        targets.live_destination.set(local_destination);
                        targets.remembered_destination.set(None);
                        targets.clone_host_state.set(CloneHostState::UserTookOver);
                        "~".to_string()
                    }
                };
                targets.choices.promote_history();
                targets.invalidate_browse();
                let changed = draft.choose_existing_folder(&path);
                (
                    true,
                    EditFacts {
                        publish: true,
                        changed: changed || correcting_invalid || host_action,
                    },
                )
            })
            .unwrap_or(false);
        // Picker completion follows key settlement, as on the original path.
        if accepted {
            ui.open.set(false);
            ui.active.set(None);
        }
        accepted
    }

    /// A direct recent applies its captured history, not a newer fetched ranking.
    /// True means admitted even on equality, so Enter can submit the completed
    /// Agent setup. Command fields and one-time inputs remain retained. The old
    /// escalation notice is cleared; this direct route adds no switch observer.
    #[expect(
        clippy::too_many_arguments,
        reason = "captured history and existing UI lifetimes are independent inputs"
    )]
    pub(super) fn accept_recent(
        self,
        entry: api::LaunchHistoryEntry,
        catalog: &[api::LaunchCatalogModel],
        captured_target: Option<CreateTarget>,
        history: Option<(CreateTarget, api::LaunchHistory)>,
        mut targets: PlacementTargets,
        mut notice: Signal<Option<PermissionEscalation>>,
        mut reset_reason: Signal<Option<String>>,
    ) -> bool {
        if self.snapshot().is_restart() {
            reset_reason.set(Some(
                "Destination cannot change here: this dialog keeps destination fixed for restart"
                    .into(),
            ));
            return false;
        }
        self.apply_edit(EditRule::Ordinary, |draft| {
            let tab_changed = draft.tab() != LaunchTab::Agent;
            let correcting_invalid = targets.correcting_invalid();
            if !admit_history_destination(
                captured_target,
                targets.live_destination,
                targets.remembered_destination,
                targets.history_activation_attempts,
            ) {
                return (
                    false,
                    EditFacts {
                        publish: false,
                        changed: false,
                    },
                );
            }
            promote_history_snapshot(
                targets.choices.offered_history,
                (targets.choices.create_target)(),
                history,
            );
            targets.choices.partial_template_hint.set(false);
            let mut selection = crate::launch_composer::select_recent(&entry);
            selection.permissions = crate::launch_composer::normalized_permissions(
                selection.harness,
                selection.permissions,
            );
            notice.set(None);
            // Direct recents retain ownership from the original entry. Search
            // recents have a separate normalized-selection contract.
            let owner = crate::launch_composer::custom_model_owner(&entry.selection, catalog);
            let selection_changed = draft.restore_launch(&selection, owner);
            targets.invalidate_browse();
            let destination_changed = if let Some(repo) = entry.github_repo {
                targets.remembered_destination.set(None);
                draft.select_repository(repo);
                targets.preview_revision.with_mut(|revision| {
                    *revision = revision.checked_add(1).expect("preview revision exhausted")
                });
                true
            } else {
                draft.choose_existing_folder(entry.cwd.raw())
            };
            draft.choose_tab(LaunchTab::Agent);
            reset_reason.set(None);
            (
                true,
                EditFacts {
                    publish: true,
                    changed: tab_changed
                        || selection_changed
                        || destination_changed
                        || correcting_invalid,
                },
            )
        })
        .unwrap_or(false)
    }

    /// A browse click is counted even when busy; authority reads occur only after
    /// admission, in request/target/connection order against the live raw cwd.
    /// Stale replies cannot promote history, publish fields or retire the pair.
    pub(super) fn accept_browse_directory(
        self,
        raw_path: &str,
        authority: &BrowseAuthority,
        mut targets: PlacementTargets,
        connection: Signal<Option<u64>>,
        mut attempts: Signal<u64>,
    ) -> bool {
        attempts.with_mut(|count| *count = count.wrapping_add(1));
        self.apply_edit(EditRule::Ordinary, |draft| {
            if !browse_activation_is_current(
                targets.browse_request,
                authority,
                targets.choices.create_target,
                connection,
                draft,
            ) {
                return (
                    false,
                    EditFacts {
                        publish: false,
                        changed: false,
                    },
                );
            }
            targets.choices.promote_history();
            let correcting_invalid = targets.correcting_invalid();
            let changed = draft.choose_existing_folder(raw_path);
            targets.remembered_destination.set(None);
            targets.invalidate_browse();
            (
                true,
                EditFacts {
                    publish: true,
                    changed: changed || correcting_invalid,
                },
            )
        })
        .unwrap_or(false)
    }

    /// Capture the actual switch boundary once, with no executable recovery.
    fn capture_permission_switch(
        self,
        draft: &LaunchDraft,
        targets: RecoveryTargets,
    ) -> PermissionSwitchBefore {
        PermissionSwitchBefore {
            harness: draft.harness(),
            permission: draft
                .harness()
                .and_then(|h| h.request_permission(draft.permissions())),
            tab: draft.tab(),
            undo: std::rc::Rc::new(PermissionUndo {
                draft: draft.clone(),
                composer_reset_reason: targets.composer_reset_reason.peek().clone(),
                chosen_host: *self.chosen_host.peek(),
                clone_host_state: *targets.clone_host_state.peek(),
                remembered_destination: targets.remembered_destination.peek().clone(),
                model_draft: targets.model_draft.peek().clone(),
                model_draft_error: targets.model_draft_error.peek().clone(),
            }),
        }
    }

    /// Direct harness switches capture before history promotion. The renderer
    /// observes the completed switch only after publication and key settlement.
    pub(super) fn choose_harness(
        self,
        harness: LaunchHarness,
        catalog: Option<&[api::LaunchCatalogModel]>,
        preferences: SharedPreferences,
        mut choices: ChoiceTargets,
        mut recovery: RecoveryTargets,
    ) -> Option<PermissionSwitchBefore> {
        if self.snapshot().is_restart()
            && (self.snapshot().tab() != LaunchTab::Agent
                || self.snapshot().harness() != Some(harness))
        {
            recovery.composer_reset_reason.set(Some(
                "Agent type cannot change here: this dialog keeps the session's agent type".into(),
            ));
            return None;
        }
        self.apply_edit(EditRule::Ordinary, |draft| {
            choices.partial_template_hint.set(false);
            let before = self.capture_permission_switch(draft, recovery);
            choices.promote_history();
            let outcome = draft.choose_harness(
                harness,
                catalog,
                remembered_wrapper(&preferences.0.peek(), Some(harness)),
                ChoiceOrigin::Control,
            );
            let changed = outcome.selection_changed || draft.tab() != LaunchTab::Agent;
            recovery.composer_reset_reason.set(outcome.reset_reason);
            draft.choose_tab(LaunchTab::Agent);
            (
                before,
                EditFacts {
                    publish: true,
                    changed,
                },
            )
        })
    }

    /// Catalog selection's capture follows history/hint and precedes cleanup.
    /// Keep this distinct from direct harness capture and Search reconciliation.
    pub(super) fn choose_catalog_model(
        self,
        id: String,
        harness: LaunchHarness,
        catalog: Option<&[api::LaunchCatalogModel]>,
        preferences: SharedPreferences,
        mut choices: ChoiceTargets,
        mut recovery: RecoveryTargets,
    ) -> Option<PermissionSwitchBefore> {
        if self.snapshot().is_restart()
            && (self.snapshot().tab() != LaunchTab::Agent
                || self.snapshot().harness() != Some(harness))
        {
            recovery.composer_reset_reason.set(Some(
                "Agent type cannot change here: this dialog keeps the session's agent type".into(),
            ));
            return None;
        }
        self.apply_edit(EditRule::Ordinary, |draft| {
            choices.promote_history();
            choices.partial_template_hint.set(false);
            let before = self.capture_permission_switch(draft, recovery);
            let outcome = draft.choose_catalog_model(
                id,
                harness,
                catalog,
                remembered_wrapper(&preferences.0.peek(), Some(harness)),
                ChoiceOrigin::Control,
            );
            recovery.composer_reset_reason.set(outcome.reset_reason);
            (
                before,
                EditFacts {
                    publish: true,
                    changed: outcome.selection_changed,
                },
            )
        })
    }

    /// Default/custom equality still completes the current picker interaction.
    pub(super) fn choose_default_model(
        self,
        mut choices: ChoiceTargets,
        mut ui: ModelTargets,
    ) -> bool {
        self.apply_ordinary(|draft| {
            choices.promote_history();
            choices.partial_template_hint.set(false);
            let changed = draft.choose_default_model();
            ui.reset_reason.set(None);
            ui.complete();
            changed
        })
    }

    /// Custom acceptance does not gain hint clearing or permission observation.
    pub(super) fn choose_custom_model(
        self,
        id: String,
        harness: LaunchHarness,
        catalog: Option<&[api::LaunchCatalogModel]>,
        choices: ChoiceTargets,
        mut ui: ModelTargets,
    ) -> bool {
        self.apply_ordinary(|draft| {
            choices.promote_history();
            let outcome = draft.choose_custom_model(id, harness, catalog);
            ui.reset_reason.set(outcome.reset_reason);
            ui.complete();
            outcome.selection_changed
        })
    }

    /// An admitted missing-harness error explicitly ends ordinary authority.
    /// No unchanged-valid request is asserted and no semantic field is published.
    pub(super) fn report_model_harness_required(self, mut error: Signal<Option<String>>) -> bool {
        self.apply_edit(EditRule::Ordinary, |_| {
            error.set(Some("Choose a harness before a custom model id".into()));
            (
                (),
                EditFacts {
                    publish: false,
                    changed: true,
                },
            )
        })
        .is_some()
    }

    /// Create resets the agent group to preference memory. Restart instead
    /// restores its stored baseline and clears the one-time conversation;
    /// clearing the held agent would leave a draft it can never submit.
    /// Both retire confirmation identity even when no field changes.
    pub(super) fn reset_choices(
        self,
        preferences: SharedPreferences,
        mut choices: ChoiceTargets,
        mut reset: Signal<Option<String>>,
        mut search: Signal<String>,
        mut search_open: Signal<bool>,
    ) -> bool {
        self.apply_edit(EditRule::Ordinary, |draft| {
            choices.partial_template_hint.set(false);
            if let Some(api::RestartEdit::Agent(selection)) = draft.restart_baseline.clone() {
                draft.restore_launch(
                    &selection,
                    selection.model.as_ref().map(|_| selection.harness),
                );
                draft.permissions = selection.permissions;
                draft.conversation.clear();
            } else {
                draft.reset_choices(initial_structured_permissions(&preferences.0.peek()));
            }
            reset.set(None);
            choices.promote_history();
            search.set(String::new());
            search_open.set(false);
            (
                (),
                EditFacts {
                    publish: true,
                    changed: true,
                },
            )
        })
        .is_some()
    }

    /// Undo obtains its payload only from the current fenced notice. Expired or
    /// superseded openings refuse before busy; restored semantics never restore
    /// preview/browse evidence or synchronously rewind live_destination.
    pub(super) fn undo_permission_switch(
        mut self,
        generation: u64,
        mut recovery: RecoveryTargets,
        mut notice: Signal<Option<PermissionEscalation>>,
        opening: Signal<u64>,
        mut placement: PlacementTargets,
    ) -> bool {
        let current = notice.peek().clone();
        let Some(current) = current.filter(|current| {
            current.generation == generation && current.opening_generation == *opening.peek()
        }) else {
            return false;
        };
        let accepted = self
            .apply_edit(EditRule::Ordinary, |draft| {
                let before = &current.undo;
                draft.restore(before.draft.clone());
                recovery
                    .composer_reset_reason
                    .set(before.composer_reset_reason.clone());
                self.chosen_host.set(before.chosen_host);
                recovery.clone_host_state.set(before.clone_host_state);
                recovery
                    .remembered_destination
                    .set(before.remembered_destination.clone());
                recovery.model_draft.set(before.model_draft.clone());
                recovery
                    .model_draft_error
                    .set(before.model_draft_error.clone());
                (
                    (),
                    EditFacts {
                        publish: true,
                        changed: true,
                    },
                )
            })
            .is_some();
        if accepted {
            placement.invalidate_browse();
            placement.preview_revision += 1;
            notice.set(None);
        }
        accepted
    }

    /// One admitted acceptance owns validation/replay and its fixed pair policy.
    /// Template refusals still earn outer completion; successful templates never
    /// clear/recreate even a mismatched complete pair. Busy earns no preamble.
    pub(super) fn accept_search_result(
        self,
        result: crate::launch_composer::ComposerSearchResult,
        captured: Option<&farhelm_proto::launcher::LaunchTemplate>,
        from_shortcut: bool,
        inputs: SearchInputs<'_>,
        mut targets: SearchTargets,
    ) -> Option<SearchAcceptance> {
        use crate::launch_composer::ComposerSearchResult;
        let is_template = matches!(&result, ComposerSearchResult::Template(_));
        self.apply_edit(
            if is_template {
                EditRule::Template
            } else {
                EditRule::Ordinary
            },
            |draft| {
                let before = self.capture_permission_switch(draft, targets.recovery);
                let templates = captured
                    .map(std::slice::from_ref)
                    .unwrap_or(inputs.templates);
                if captured.is_none() || from_shortcut {
                    targets.placement.choices.partial_template_hint.set(false);
                }
                let automatic_permission = match &result {
                    ComposerSearchResult::Template(name) => templates
                        .iter()
                        .find(|t| &t.name == name)
                        .is_some_and(|t| t.fields.permissions.is_none()),
                    ComposerSearchResult::Recent(_) | ComposerSearchResult::Permissions(_) => false,
                    _ => true,
                };
                targets.placement.invalidate_browse();
                targets.placement.choices.promote_history();
                let (browse_path, publish, changed) = if let ComposerSearchResult::Template(name) =
                    result
                {
                    let applied = match templates.iter().find(|t| t.name == name) {
                        Some(template) => self.apply_template(draft, template, &inputs, targets),
                        None => Err(format!(
                            "No template is named {name:?}; it may have been deleted"
                        )),
                    };
                    match applied {
                        Ok(()) => (None, true, false),
                        Err(message) => {
                            targets.template_error.set(Some(message));
                            (None, false, false)
                        }
                    }
                } else {
                    targets.template_error.set(None);
                    let (path, changed, publish) =
                        self.apply_search_result(draft, result, &inputs, targets);
                    (path, publish, changed)
                };
                (
                    SearchAcceptance {
                        before,
                        automatic_permission,
                        browse_path,
                        is_template,
                    },
                    EditFacts { publish, changed },
                )
            },
        )
    }

    /// Whole validation precedes every semantic write. Private search branches
    /// retain their conditional observations but never recursively gate/publish.
    fn apply_template(
        self,
        draft: &mut LaunchDraft,
        template: &farhelm_proto::launcher::LaunchTemplate,
        inputs: &SearchInputs<'_>,
        mut targets: SearchTargets,
    ) -> Result<(), String> {
        use crate::launch_composer::{ComposerPermission, ComposerSearchResult};
        use farhelm_proto::launcher::{LauncherKind, TemplateDestination};
        let (applied, repositories, templates) = super::validated_template_batch(
            std::slice::from_ref(template),
            inputs.hosts,
            inputs.catalog.unwrap_or_default(),
            inputs.host_fixed,
            draft,
            *self.chosen_host.peek(),
        )?;
        let repository = repositories.into_iter().next().flatten();
        targets.template_error.set(None);
        // Replay the same cleaned fields the fold accepted. Using the stored
        // template here would reapply held choices after validation removed them.
        let fields = templates
            .into_iter()
            .next()
            .expect("one template validated")
            .fields;
        let kind = applied.kind.unwrap_or(LauncherKind::Agent);
        // Kind precedes agent interpretation: the same agent field means
        // a structured choice or a command declaration depending on this step.
        if let Some(kind) = fields.kind {
            draft.choose_tab(match kind {
                LauncherKind::Agent => LaunchTab::Agent,
                LauncherKind::Command => LaunchTab::Command,
            });
        }
        if let Some(agent) = fields.agent {
            match kind {
                LauncherKind::Agent => {
                    self.apply_search_result(
                        draft,
                        ComposerSearchResult::Harness(agent),
                        inputs,
                        targets,
                    );
                }
                LauncherKind::Command => {
                    draft.choose_command_agent(Some(agent));
                }
            }
        }
        // Resets retain their direct-field semantics; actual picks keep the
        // private search branch's reconciliation and conditional subscriptions.
        match fields.model {
            Some(Some(id)) => {
                self.apply_search_result(
                    draft,
                    ComposerSearchResult::Model {
                        id,
                        harness: applied
                            .harness
                            .expect("a model applies only with an agent type"),
                    },
                    inputs,
                    targets,
                );
            }
            Some(None) => {
                draft.choose_default_model();
                targets.recovery.composer_reset_reason.set(None);
            }
            None => {}
        }
        match fields.effort {
            Some(Some(effort)) => {
                self.apply_search_result(
                    draft,
                    ComposerSearchResult::Effort(Some(effort)),
                    inputs,
                    targets,
                );
            }
            Some(None) => {
                draft.choose_effort(None);
            }
            None => {}
        }
        let flags = fields
            .permission_flags
            .as_ref()
            .map(shell_words::join)
            .unwrap_or_default();
        match fields.permissions {
            Some(Some(LaunchPermission::Yolo)) => {
                self.apply_search_result(
                    draft,
                    ComposerSearchResult::Permissions(ComposerPermission::Yolo),
                    inputs,
                    targets,
                );
            }
            Some(None) => draft.apply_template_permissions(None, String::new()),
            Some(Some(permission)) => draft.apply_template_permissions(Some(permission), flags),
            None => {}
        }
        if let Some(prefix) = fields.wrapper_prefix {
            draft.edit_wrapper(prefix.as_ref().map(shell_words::join).unwrap_or_default());
        }
        // Template-owned command text deliberately discards peer raw seeds.
        if let Some(command) = fields.command {
            draft.apply_template_invocation(command);
        }
        if let Some(yolo) = fields.yolo {
            draft.choose_command_yolo(yolo);
        }
        if let Some(resume) = fields.resume_command {
            draft.apply_template_resume(resume);
        }
        // Host/destination use existing private actions. Names additionally
        // retain Base/Fixed policy and invalidate even on a kind-only change.
        if let Some(identity) = &fields.host
            && let Some(host) = super::template_host(inputs.hosts, identity)
        {
            self.apply_search_result(
                draft,
                ComposerSearchResult::Host(crate::launch_composer::ComposerHost {
                    id: host.id,
                    name: host.name.clone(),
                    label: host.label(),
                    local: host.local,
                }),
                inputs,
                targets,
            );
        }
        match (fields.destination, repository) {
            (Some(TemplateDestination::Folder(folder)), _) => {
                self.apply_search_result(
                    draft,
                    ComposerSearchResult::UsePath(folder),
                    inputs,
                    targets,
                );
            }
            (Some(TemplateDestination::Github(_)), Some(repo)) => {
                self.apply_search_result(
                    draft,
                    ComposerSearchResult::Github(repo),
                    inputs,
                    targets,
                );
            }
            _ => {}
        }
        if let Some(name) = fields.name {
            draft.apply_template_name(
                name,
                applied
                    .name_kind
                    .expect("an applied name carries its template policy"),
            );
            targets.placement.preview_revision.with_mut(|revision| {
                *revision = revision.checked_add(1).expect("preview revision exhausted")
            });
        }
        // Shared application can clear an incompatible dormant message. Its
        // final presence, including Some(empty), is part of completed replay.
        draft.apply_template_prompt(applied.prompt);
        Ok(())
    }

    /// Search and template replay share this finite plain application. Inner
    /// actions never publish or settle the pair; the outer owner rule does that.
    /// Return path, retirement fact and publication separately: stale history
    /// refuses publication, equal effort still assigns, and Host is external.
    fn apply_search_result(
        self,
        draft: &mut LaunchDraft,
        result: crate::launch_composer::ComposerSearchResult,
        inputs: &SearchInputs<'_>,
        targets: SearchTargets,
    ) -> (Option<String>, bool, bool) {
        let catalog_answer = inputs.catalog;
        let catalog = catalog_answer.unwrap_or_default();
        let hosts = inputs.hosts;
        let preferences = inputs.preferences;
        let history_target = inputs.history_target.clone();
        let mut chosen_host = self.chosen_host;
        let mut clone_host_state = targets.placement.clone_host_state;
        let mut live_destination = targets.placement.live_destination;
        let mut remembered_destination = targets.placement.remembered_destination;
        let history_activation_attempts = targets.placement.history_activation_attempts;
        let mut preview_revision = targets.placement.preview_revision;
        let mut composer_reset_reason = targets.recovery.composer_reset_reason;
        let mut action_error = targets.template_error;
        use crate::launch_composer::ComposerSearchResult;
        if draft.is_restart() {
            let held = match &result {
                ComposerSearchResult::Harness(_) => Some("agent type"),
                ComposerSearchResult::Model { harness, .. }
                    if draft.tab != LaunchTab::Agent || draft.harness != Some(*harness) =>
                {
                    Some("agent type")
                }
                ComposerSearchResult::Host(_) => Some("host"),
                ComposerSearchResult::Folder(_)
                | ComposerSearchResult::UsePath(_)
                | ComposerSearchResult::BrowsePath(_)
                | ComposerSearchResult::Github(_) => Some("destination"),
                ComposerSearchResult::Name(_) => Some("session name"),
                ComposerSearchResult::Recent(_) => Some("destination"),
                _ => None,
            };
            if let Some(field) = held {
                action_error.set(Some(format!(
                    "{field} cannot change here: this dialog keeps {field} fixed for restart"
                )));
                return (None, false, false);
            }
        }
        // Capture before history adoption or path override can clear the old claim.
        let correcting_invalid = !remembered_destination_matches(
            remembered_destination.peek().as_ref(),
            live_destination.peek().as_ref(),
        );
        let changed;
        if matches!(
            &result,
            ComposerSearchResult::Folder(_) | ComposerSearchResult::Recent(_)
        ) && !admit_history_destination(
            history_target,
            live_destination,
            remembered_destination,
            history_activation_attempts,
        ) {
            // A retained result can outlive its installation. Keep the existing
            // admission fence, but explain the rejected activation where other
            // search/template failures already appear.
            action_error.set(Some(
                "That result belongs to an earlier host; choose a current result".into(),
            ));
            return (None, false, false);
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
            // here (apply_template), and none of those edits is a template.
            ComposerSearchResult::Template(_) => {
                unreachable!("apply_template expands every template result")
            }
            ComposerSearchResult::Name(name) => {
                // A cloned title may have an escaped display seed. This action
                // is the user's new text, so submit and checkout preview must
                // read it rather than replay the clone's raw title.
                changed = draft.edit_title(name);
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
                return (None, true, false);
            }
            ComposerSearchResult::Github(repo) => {
                changed = true;
                remembered_destination.set(None);
                draft.select_repository(repo);
                preview_revision.with_mut(|value| {
                    *value = value.checked_add(1).expect("preview revision exhausted")
                });
            }
            crate::launch_composer::ComposerSearchResult::UsePath(folder)
            | crate::launch_composer::ComposerSearchResult::Folder(folder) => {
                changed = draft.choose_existing_folder(&folder) || correcting_invalid;
            }
            crate::launch_composer::ComposerSearchResult::BrowsePath(folder) => {
                // Query-path actions are treated like other relayed path choices:
                // the escaped field remains reviewable, while the browse request
                // and an untouched later create retain the exact requested bytes.
                let changed = draft.choose_existing_folder(&folder) || correcting_invalid;
                return (Some(folder), changed, true);
            }
            crate::launch_composer::ComposerSearchResult::Harness(harness) => {
                let tab_changed = draft.tab() != LaunchTab::Agent;
                draft.choose_tab(LaunchTab::Agent);
                observe_search_agent_fields(self.fields());
                let outcome = draft.choose_harness(
                    harness,
                    catalog_answer,
                    remembered_wrapper(&preferences.0.peek(), Some(harness)),
                    ChoiceOrigin::Search,
                );
                changed = outcome.selection_changed || tab_changed;
                composer_reset_reason.set(outcome.reset_reason);
            }
            crate::launch_composer::ComposerSearchResult::Model { id, harness } => {
                let tab_changed = draft.tab() != LaunchTab::Agent;
                // A model carries its harness ownership, so selecting it is also
                // an explicit return to structured mode rather than leaving a
                // structured draft hidden behind command controls.
                draft.choose_tab(LaunchTab::Agent);
                observe_search_agent_fields(self.fields());
                let outcome = draft.choose_catalog_model(
                    id,
                    harness,
                    catalog_answer,
                    remembered_wrapper(&preferences.0.peek(), Some(harness)),
                    ChoiceOrigin::Search,
                );
                changed = outcome.selection_changed || tab_changed;
                composer_reset_reason.set(outcome.reset_reason);
            }
            crate::launch_composer::ComposerSearchResult::Effort(effort) => {
                // An unchanged choice is a retry gesture, just like pressing its
                // button again. Preserve the key after an ambiguous reply so that
                // accepting effort:default cannot create a second session. A real
                // edit reaches the shared key reset below; harness and model stay
                // put, so no reconciliation runs.
                if !draft.choose_effort(effort) {
                    return (None, false, true);
                }
                changed = true;
            }
            crate::launch_composer::ComposerSearchResult::Permissions(permission) => {
                let focus_custom = permission == crate::launch_composer::ComposerPermission::Custom;
                let permission = match permission {
                    crate::launch_composer::ComposerPermission::Custom => {
                        Some(LaunchPermission::Custom)
                    }
                    crate::launch_composer::ComposerPermission::Guarded => {
                        Some(LaunchPermission::Guarded)
                    }
                    crate::launch_composer::ComposerPermission::OwnSetting => None,
                    crate::launch_composer::ComposerPermission::Yolo => {
                        Some(LaunchPermission::Yolo)
                    }
                    crate::launch_composer::ComposerPermission::Approve => {
                        Some(LaunchPermission::Approve)
                    }
                    crate::launch_composer::ComposerPermission::SmartApprove => {
                        Some(LaunchPermission::SmartApprove)
                    }
                    crate::launch_composer::ComposerPermission::Chat => {
                        Some(LaunchPermission::Chat)
                    }
                };
                changed = draft.choose_permissions(permission);
                if focus_custom {
                    document::eval(
                        "requestAnimationFrame(() => document.querySelector('.custom-permission-flags')?.focus())",
                    );
                }
            }
            crate::launch_composer::ComposerSearchResult::Recent(entry) => {
                let tab_changed = draft.tab() != LaunchTab::Agent;
                draft.choose_tab(LaunchTab::Agent);
                composer_reset_reason.set(None);
                let mut selection = crate::launch_composer::select_recent(&entry);
                selection.permissions = crate::launch_composer::normalized_permissions(
                    selection.harness,
                    selection.permissions,
                );
                let owner = crate::launch_composer::custom_model_owner(&selection, catalog);
                let selection_changed = draft.restore_launch(&selection, owner);
                if let Some(repo) = entry.github_repo {
                    changed = true;
                    remembered_destination.set(None);
                    draft.select_repository(repo);
                    preview_revision.with_mut(|value| {
                        *value = value.checked_add(1).expect("preview revision exhausted")
                    });
                } else {
                    changed = draft.choose_existing_folder(entry.cwd.raw())
                        || selection_changed
                        || tab_changed
                        || correcting_invalid;
                }
            }
        }
        (None, changed, true)
    }

    /// Seed launch fields once per source generation, but resolve its host every pass.
    /// Keep the entry target observation separate from a host chosen later in this
    /// invocation: target retirement catches up on the next pass as before. Both
    /// source retirement phases remain available under a live operation claim.
    pub(super) fn reconcile_source(
        mut self,
        prefill: Option<&CreatePrefill>,
        hosts_loaded: bool,
        hosts: &[HostOption],
        mut targets: SourceTargets,
    ) {
        // Even an already-seeded source supersedes waiting opening work. Read
        // the tracked target only after this every-pass preamble, never while
        // constructing targets or after clone host resolution has changed it.
        if prefill.is_some() {
            targets.pending_initial_action.set(None);
            targets.pending_initial_templates.write().clear();
            targets.partial_template_hint.set(false);
            targets.auto_launch_pending.set(false);
            targets.quick_launch_hidden.set(false);
        }
        let target = (targets.create_target)();
        let previous = targets.bound_target.peek().clone();
        if let Some(prefill) =
            prefill.filter(|prefill| Some(prefill.generation) != *targets.prefill_applied.peek())
        {
            targets.prefill_applied.set(Some(prefill.generation));
            let _ = self.apply_edit(EditRule::SourceReconciliation, |draft| {
                draft.clear_prompt_for_prefill();
                draft.restart_baseline = match &prefill.mode {
                    super::LauncherMode::Restart { baseline, .. } => Some(baseline.clone()),
                    _ => None,
                };
                targets.permission_escalation.set(None);
                // Invalidate before seeding: A→B→A source changes must never
                // revive an old directory listing whose raw path matches again.
                invalidate_directory_browse(
                    targets.browse_generation,
                    targets.browse_request,
                    targets.browse_result,
                    targets.browse_error,
                );
                draft.seed_folder(&prefill.cwd);
                targets.template_panel.set(None);
                let source_repo = prefill
                    .repo
                    .clone()
                    .filter(|_| !prefill.mode.seeds_source_folder());
                draft.seed_title(&prefill.title, source_repo.clone());
                if let Some(repo) = source_repo {
                    draft.select_repository(repo);
                    targets.preview_revision.with_mut(|revision| {
                        *revision = revision.checked_add(1).expect("preview revision exhausted")
                    });
                }
                targets.remembered_destination.set(None);
                // Invocation and declared command metadata have distinct source
                // ownership. A structured source also seeds its dormant command.
                draft.seed_invocation(&prefill.invocation);
                draft.seed_command_fields(prefill.command.as_ref());
                if let Some(launch) = &prefill.launch {
                    draft.choose_tab(LaunchTab::Agent);
                    draft.restore_launch(launch, launch.model.as_ref().map(|_| launch.harness));
                    if draft.is_restart() {
                        // Create's default normalization is intentional for
                        // Clone, but Restart compares the stored command.
                        draft.permissions = launch.permissions;
                    }
                } else {
                    draft.choose_tab(LaunchTab::Command);
                    draft.clear_for_command_seed();
                }
                (
                    (),
                    EditFacts {
                        publish: true,
                        changed: true,
                    },
                )
            });
            // The seed is published and its key retired before old refusal
            // cleanup and host resolution, independently of target retirement.
            targets.error.set(None);
            targets.preparation_error.set(None);
            targets.composer_reset_reason.set(None);
            self.chosen_host.set(None);
            targets.clone_host_state.set(match prefill.host {
                None => CloneHostState::Unconfirmable,
                Some(_) => CloneHostState::Waiting,
            });
        }
        // Delayed host reads and withdrawal cannot pass through the generation
        // latch again: doing so would overwrite a literal edit made meanwhile.
        if let Some(prefill) = prefill {
            let (next_state, action) = resolve_clone_host(
                *targets.clone_host_state.peek(),
                prefill.host,
                &prefill.host_identity,
                hosts_loaded,
                hosts,
                prefill.host.is_some() && self.chosen_host.peek().as_ref() == prefill.host.as_ref(),
            );
            targets.clone_host_state.set(next_state);
            match action {
                CloneHostAction::Hold => {}
                CloneHostAction::Bind(target) => self.chosen_host.set(Some(target.host)),
                CloneHostAction::Withdraw => self.chosen_host.set(None),
            }
        }
        targets.mount_seeded.set(true);
        if previous != target {
            targets.bound_target.set(target);
            let _ = self.apply_edit(EditRule::SourceReconciliation, |_| {
                (
                    (),
                    EditFacts {
                        publish: false,
                        changed: true,
                    },
                )
            });
            targets.error.set(None);
        }
    }

    /// Resolve naming against the synchronous destination before preview construction.
    /// Compare only the existing naming value; equal renders publish nothing and
    /// a real reset never retires an ordinary pair or a retained checkout attempt.
    pub(super) fn retarget_checkout_name(self, destination_now: Option<CreateTarget>) {
        let _ = self.apply_edit(EditRule::CheckoutNaming, |draft| {
            let before = draft.checkout_naming();
            draft.retarget_naming(destination_now);
            let publish = before != draft.checkout_naming();
            (
                (),
                EditFacts {
                    publish,
                    changed: false,
                },
            )
        });
    }

    /// Only an occupied reply for the current default may advance its sequence.
    /// The caller supplies its already-captured authority pair and retained-match
    /// observation. Stale replies, fixed/manual names, caps and retained attempts
    /// publish nothing; the returned advance preserves the effect's early return.
    pub(super) fn advance_checkout_name_for_occupied_reply(
        self,
        authority: &PreviewAuthority,
        live: &PreviewAuthority,
        failure: &api::GithubPreviewFailure,
        reconciling: bool,
    ) -> bool {
        self.apply_edit(EditRule::CheckoutNaming, |draft| {
            if authority != live {
                return (
                    false,
                    EditFacts {
                        publish: false,
                        changed: false,
                    },
                );
            }
            let default = draft.checkout_naming().default_title(
                draft.title_edited(),
                draft.title_raw_seed().as_deref(),
                draft.destination().repo(),
            );
            let advanced = failure.occupied
                && default.as_ref() == authority.title.as_ref()
                && default.is_some()
                && draft.advance_naming(reconciling);
            (
                advanced,
                EditFacts {
                    publish: advanced,
                    changed: false,
                },
            )
        })
        .unwrap_or(false)
    }
}

/// Mount one draft and equality-filtered projections of its accepted fields.
/// Only these memos subscribe to the aggregate; effects subscribe to the
/// individual values they already observed, not every unrelated edit. Initial
/// cwd is a mount-only raw seed; later source changes use explicit transitions.
/// Operation/retry and parent host-selection handles retain their existing
/// identities; this hook recreates or mirrors none of those live authorities.
pub(super) fn use_launch_draft(
    permission: Option<LaunchPermission>,
    initial_cwd: Option<String>,
    ops: OpLock,
    intent_key: Signal<Option<(String, IntentBinding)>>,
    chosen_host: Signal<Option<HostId>>,
) -> LaunchDraftState {
    let value = use_signal(|| {
        let mut draft = LaunchDraft::new(permission);
        if let Some(raw) = initial_cwd {
            draft.seed_folder(&raw);
        }
        draft
    });
    let fields = DraftFields {
        cwd: use_memo(move || value.read().cwd()),
        cwd_edited: use_memo(move || value.read().cwd_edited()),
        title: use_memo(move || value.read().title()),
        title_raw_seed: use_memo(move || value.read().title_raw_seed()),
        title_edited: use_memo(move || value.read().title_edited()),
        destination_draft: use_memo(move || value.read().destination()),
        clone_checkout_naming: use_memo(move || value.read().checkout_naming()),
        launch_tab: use_memo(move || value.read().tab()),
        invocation: use_memo(move || value.read().invocation()),
        invocation_edited: use_memo(move || value.read().invocation_edited()),
        command_yolo: use_memo(move || value.read().command_yolo()),
        command_agent: use_memo(move || value.read().command_agent()),
        command_resume_on: use_memo(move || value.read().command_resume_on()),
        command_resume: use_memo(move || value.read().command_resume()),
        command_resume_edited: use_memo(move || value.read().command_resume_edited()),
        prompt: use_memo(move || value.read().prompt()),
        conversation: use_memo(move || value.read().conversation()),
        structured_harness: use_memo(move || value.read().harness()),
        structured_model: use_memo(move || value.read().model()),
        structured_model_raw_seed: use_memo(move || value.read().model_raw_seed()),
        structured_model_edited: use_memo(move || value.read().model_edited()),
        structured_effort: use_memo(move || value.read().effort()),
        structured_permissions: use_memo(move || value.read().permissions()),
        structured_permissions_is_explicit: use_memo(move || value.read().permissions_explicit()),
        structured_permission_flags: use_memo(move || value.read().permission_flags()),
        structured_wrapper: use_memo(move || value.read().wrapper()),
        wrapper_is_explicit: use_memo(move || value.read().wrapper_explicit()),
    };
    LaunchDraftState {
        value,
        fields,
        ops,
        intent_key,
        chosen_host,
    }
}

/// Search replay can run synchronously inside the initial-template effect.
/// Keep precisely its former structured subscriptions even though mutation
/// now reads the live plain draft. Subscribing to the aggregate here would
/// make unrelated ownership/editor changes replay pending opening work.
fn observe_search_agent_fields(fields: DraftFields) {
    let _ = (
        (fields.structured_permissions)(),
        (fields.structured_permission_flags)(),
        (fields.structured_wrapper)(),
        (fields.structured_harness)(),
        (fields.structured_model)(),
        (fields.structured_effort)(),
        (fields.structured_permissions_is_explicit)(),
    );
}

#[cfg(test)]
mod tests {
    use super::super::LauncherMode;
    use super::*;

    /// Exercise the stored-command comparison without mounting catalog readers.
    /// The production source seeder supplies this same normalized baseline.
    fn restart_draft(permission: Option<LaunchPermission>) -> LaunchDraft {
        let selection = LaunchSelection {
            harness: LaunchHarness::Codex,
            model: None,
            effort: None,
            permissions: permission,
            permission_flags: None,
            wrapper_prefix: None,
            workspace_trust: None,
        };
        let mut draft = LaunchDraft::new(None);
        draft.restart_baseline = Some(api::RestartEdit::Agent(selection.clone()));
        draft.restore_launch(&selection, None);
        draft.permissions = permission;
        draft
    }

    /// Own-setting omission is a historical command, not create's default.
    /// Guarded must be a real edit and retire confirmation identity in both
    /// directions even though ordinary creation normalizes omission to Guarded.
    #[test]
    fn restart_preserves_own_setting_and_counts_guarded_as_an_edit() {
        let mut draft = restart_draft(None);
        assert_eq!(draft.restart_edit(), draft.restart_baseline);
        assert!(draft.choose_permissions(Some(LaunchPermission::Guarded)));
        assert_ne!(draft.restart_edit(), draft.restart_baseline);
        assert!(draft.choose_permissions(None));
        assert_eq!(draft.restart_edit(), draft.restart_baseline);
    }

    /// Template null means today's request default, while an omitted field
    /// leaves a historical own-setting launch alone. Restart equality must
    /// agree with what the helm compiles, rather than enabling a no-op edit.
    #[test]
    fn restart_template_permission_reset_uses_the_current_default() {
        for stored in [None, Some(LaunchPermission::Guarded)] {
            let mut draft = restart_draft(stored);
            draft.choose_permissions(Some(LaunchPermission::Yolo));
            draft.apply_template_permissions(None, String::new());
            assert_eq!(draft.permissions(), Some(LaunchPermission::Guarded));
            assert_eq!(
                draft.restart_edit() == draft.restart_baseline,
                stored.is_some()
            );
        }
    }

    /// The Guarded answer cannot erase the only edit and submit an unchanged
    /// restart. A named conversation or another changed setting still qualifies.
    #[test]
    fn restart_guarded_offer_requires_an_edit_after_guarding() {
        let mut draft = restart_draft(Some(LaunchPermission::Guarded));
        draft.choose_permissions(Some(LaunchPermission::Yolo));
        assert!(!draft.restart_guarded_changes());
        draft.edit_conversation("chosen-id".into());
        assert!(draft.restart_guarded_changes());
        draft.edit_conversation(String::new());
        draft.choose_effort(Some(LaunchEffort::High));
        assert!(draft.restart_guarded_changes());
    }

    /// Restart keeps raw command bytes across an unrelated YOLO edit, including
    /// peer formatting that the controls must show escaped rather than execute.
    #[test]
    fn restart_command_projection_preserves_unedited_bytes() {
        let command = CommandLaunch {
            command: "claude  --flag='a b' {farhelm_args}\u{202e}".into(),
            resume: Some("claude  --resume {conversation} {farhelm_args}\u{200b}".into()),
            yolo: false,
            agent: Some(LaunchHarness::Claude),
        };
        let mut draft = LaunchDraft::new(None);
        draft.restart_baseline = Some(api::RestartEdit::Command(command.clone()));
        draft.seed_invocation(&command.command);
        draft.seed_command_fields(Some(&command));
        assert_eq!(
            draft.restart_edit(),
            Some(api::RestartEdit::Command(command.clone()))
        );
        draft.choose_command_yolo(true);
        let mut expected = command;
        expected.yolo = true;
        assert_eq!(
            draft.restart_edit(),
            Some(api::RestartEdit::Command(expected))
        );
    }

    /// A real held-field change still refuses the whole shortcut batch.
    /// An earlier ordinary refusal must win over a later held-field
    /// refusal in a shortcut batch; validation never publishes partial edits.
    #[test]
    fn restart_templates_refuse_held_fields_in_batch_order() {
        use farhelm_proto::launcher::{LaunchTemplate, LauncherKind, TemplateFields};
        let draft = restart_draft(None);
        let held = LaunchTemplate {
            name: "different kind".into(),
            fields: TemplateFields {
                kind: Some(LauncherKind::Command),
                effort: Some(Some(LaunchEffort::High)),
                ..Default::default()
            },
        };
        assert!(
            draft
                .restart_template_refusal(&held)
                .is_some_and(|text| text.contains("launch kind cannot change here"))
        );
        let invalid = LaunchTemplate {
            name: "bad command".into(),
            fields: TemplateFields {
                command: Some("not an agent field".into()),
                ..Default::default()
            },
        };
        let error =
            super::super::validated_template_batch(&[invalid, held], &[], &[], true, &draft, None)
                .unwrap_err();
        assert!(
            error.contains("bad command"),
            "the first refusal describes the first template: {error}"
        );
        assert_eq!(
            draft.restart_edit(),
            draft.restart_baseline,
            "validation did not apply the later effort"
        );
    }

    /// Each held value may be repeated without losing an accompanying edit;
    /// a different value must refuse the entire template and name the field.
    /// Raw names must not compare equal through title/display normalization.
    #[test]
    fn restart_templates_compare_held_fields_before_validation() {
        use farhelm_proto::launcher::{
            LaunchTemplate, LauncherKind, TemplateDestination, TemplateFields, TemplateNameKind,
        };
        let mut draft = restart_draft(None);
        draft.seed_folder("/work/project\u{202e}");
        draft.seed_title("work \u{202e}", None);
        let hosts = [crate::list::shared::tests::option(1, "host", true)];
        let cases = [
            (
                "launch kind",
                TemplateFields {
                    kind: Some(LauncherKind::Agent),
                    ..Default::default()
                },
                TemplateFields {
                    kind: Some(LauncherKind::Command),
                    ..Default::default()
                },
            ),
            (
                "agent type",
                TemplateFields {
                    agent: Some(LaunchHarness::Codex),
                    ..Default::default()
                },
                TemplateFields {
                    agent: Some(LaunchHarness::Claude),
                    ..Default::default()
                },
            ),
            (
                "host",
                TemplateFields {
                    host: Some("install-1".into()),
                    ..Default::default()
                },
                TemplateFields {
                    host: Some("install-other".into()),
                    ..Default::default()
                },
            ),
            (
                "destination",
                TemplateFields {
                    destination: Some(TemplateDestination::Folder("/work/project\u{202e}/".into())),
                    ..Default::default()
                },
                TemplateFields {
                    destination: Some(TemplateDestination::Folder("~/project".into())),
                    ..Default::default()
                },
            ),
            (
                "session name",
                TemplateFields {
                    name: Some("work \u{202e}".into()),
                    name_kind: Some(TemplateNameKind::Fixed),
                    ..Default::default()
                },
                TemplateFields {
                    name: Some(draft.title()),
                    ..Default::default()
                },
            ),
        ];
        for (field, equal, different) in cases {
            let template = LaunchTemplate {
                name: format!("matching {field}"),
                fields: TemplateFields {
                    effort: Some(Some(LaunchEffort::High)),
                    ..equal
                },
            };
            let (state, _, cleaned) = super::super::validated_template_batch(
                &[template],
                &hosts,
                &[],
                true,
                &draft,
                Some(1),
            )
            .unwrap();
            assert_eq!(state.effort, Some(LaunchEffort::High), "{field}");
            assert_eq!(
                cleaned[0].fields,
                TemplateFields {
                    effort: Some(Some(LaunchEffort::High)),
                    ..Default::default()
                },
                "only the editable field survives: {field}"
            );
            let template = LaunchTemplate {
                name: format!("changed {field}"),
                fields: TemplateFields {
                    effort: Some(Some(LaunchEffort::High)),
                    ..different
                },
            };
            let refusal = super::super::validated_template_batch(
                &[template],
                &hosts,
                &[],
                true,
                &draft,
                Some(1),
            )
            .unwrap_err();
            assert!(
                refusal.contains(&format!("{field} cannot change here")),
                "{refusal}"
            );
            assert_eq!(draft.restart_edit(), draft.restart_baseline);
        }
    }

    /// Dropping an equal agent is essential: reselecting it would reconcile an
    /// old unset permission to Guarded, turning a held-only template into an edit.
    /// The cleaned template is the replay contract, not merely a validation aid.
    #[test]
    fn restart_matching_agent_keeps_historical_own_settings() {
        use farhelm_proto::launcher::{LaunchTemplate, LauncherKind, TemplateFields};
        let draft = restart_draft(None);
        let before = super::super::launcher_snapshot(&draft, &[], None);
        let template = LaunchTemplate {
            name: "same launcher".into(),
            fields: TemplateFields {
                kind: Some(LauncherKind::Agent),
                agent: Some(LaunchHarness::Codex),
                ..Default::default()
            },
        };
        let (after, _, cleaned) =
            super::super::validated_template_batch(&[template], &[], &[], true, &draft, None)
                .unwrap();
        assert_eq!(after, before);
        assert_eq!(cleaned[0].fields, TemplateFields::default());
        assert_eq!(draft.restart_edit(), draft.restart_baseline);
    }

    /// Non-folder destinations have no equal-value escape, and an empty path
    /// cannot match root. Prompt and command-resume removal remain refused.
    #[test]
    fn restart_templates_keep_nonmatching_boundaries() {
        use farhelm_proto::launcher::{LaunchTemplate, TemplateDestination, TemplateFields};
        let mut draft = restart_draft(None);
        draft.seed_folder("/");
        let cases = [
            (
                "destination",
                TemplateFields {
                    destination: Some(TemplateDestination::Github("owner/repo".into())),
                    ..Default::default()
                },
            ),
            (
                "destination",
                TemplateFields {
                    destination: Some(TemplateDestination::Folder(String::new())),
                    ..Default::default()
                },
            ),
            (
                "initial prompt",
                TemplateFields {
                    prompt: Some(String::new()),
                    ..Default::default()
                },
            ),
        ];
        for (field, fields) in cases {
            let template = LaunchTemplate {
                name: field.into(),
                fields,
            };
            let refusal =
                super::super::validated_template_batch(&[template], &[], &[], true, &draft, None)
                    .unwrap_err();
            assert!(
                refusal.contains(&format!("{field} cannot change here")),
                "{refusal}"
            );
        }
        let command = CommandLaunch {
            command: "tool".into(),
            resume: Some("tool {conversation}".into()),
            yolo: false,
            agent: None,
        };
        draft.restart_baseline = Some(api::RestartEdit::Command(command.clone()));
        draft.seed_invocation(&command.command);
        draft.seed_command_fields(Some(&command));
        for (field, fields) in [
            (
                "agent type",
                TemplateFields {
                    agent: Some(LaunchHarness::Codex),
                    ..Default::default()
                },
            ),
            (
                "resume choice",
                TemplateFields {
                    resume_command: Some(None),
                    ..Default::default()
                },
            ),
        ] {
            let template = LaunchTemplate {
                name: field.into(),
                fields,
            };
            let refusal =
                super::super::validated_template_batch(&[template], &[], &[], true, &draft, None)
                    .unwrap_err();
            assert!(
                refusal.contains(&format!("{field} cannot change here")),
                "{refusal}"
            );
        }
        draft.command_agent = Some(LaunchHarness::Codex);
        let template = LaunchTemplate {
            name: "same command agent".into(),
            fields: TemplateFields {
                agent: Some(LaunchHarness::Codex),
                yolo: Some(true),
                ..Default::default()
            },
        };
        let (state, _, cleaned) =
            super::super::validated_template_batch(&[template], &[], &[], true, &draft, None)
                .unwrap();
        assert_eq!(cleaned[0].fields.agent, None);
        assert_eq!(state.yolo, Some(true));
    }

    /// Replace may reuse its own host without replaying a host transition;
    /// missing identity or mismatch cannot establish that equality in either mode.
    /// New keeps ordinary host assignments, including an equal current host.
    #[test]
    fn held_template_hosts_require_a_usable_matching_installation() {
        use farhelm_proto::launcher::{LaunchTemplate, TemplateFields};
        let mut host = crate::list::shared::tests::option(1, "host", true);
        let template = LaunchTemplate {
            name: "host".into(),
            fields: TemplateFields {
                host: host.identity.clone(),
                ..Default::default()
            },
        };
        for restart in [false, true] {
            let draft = if restart {
                restart_draft(None)
            } else {
                LaunchDraft::new(None)
            };
            let (_, _, cleaned) = super::super::validated_template_batch(
                std::slice::from_ref(&template),
                std::slice::from_ref(&host),
                &[],
                true,
                &draft,
                Some(1),
            )
            .unwrap();
            assert_eq!(cleaned[0].fields.host, None);
            for unusable in [None, Some("other-install".into())] {
                host.identity = unusable;
                let refusal = super::super::validated_template_batch(
                    std::slice::from_ref(&template),
                    std::slice::from_ref(&host),
                    &[],
                    true,
                    &draft,
                    Some(1),
                )
                .unwrap_err();
                assert!(refusal.contains("host cannot change here"), "{refusal}");
            }
            host.identity = template.fields.host.clone();
            host.identity_mismatch = true;
            assert!(
                super::super::validated_template_batch(
                    std::slice::from_ref(&template),
                    std::slice::from_ref(&host),
                    &[],
                    true,
                    &draft,
                    Some(1)
                )
                .is_err()
            );
            host.identity_mismatch = false;
        }
        let (_, _, cleaned) = super::super::validated_template_batch(
            std::slice::from_ref(&template),
            &[host],
            &[],
            false,
            &LaunchDraft::new(None),
            Some(1),
        )
        .unwrap();
        assert_eq!(cleaned[0].fields.host, template.fields.host);
    }

    /// Give deliberate choices distinguishable offered/fetched installation data.
    /// Real signals keep promotion and effect subscriptions observable without
    /// rebuilding those rules in a fixture-side edit implementation.
    fn use_choice_targets() -> ChoiceTargets {
        let target = CreateTarget::new(1, "install-a".into());
        ChoiceTargets {
            partial_template_hint: use_signal(|| true),
            offered_history: use_signal({
                let target = target.clone();
                move || {
                    Some((
                        target,
                        api::LaunchHistory {
                            checkout_config_revision: 1,
                            ..Default::default()
                        },
                    ))
                }
            }),
            create_target: use_signal({
                let target = target.clone();
                move || Some(target)
            }),
            fetched_history: use_signal(move || {
                Some((
                    target,
                    api::LaunchHistory {
                        checkout_config_revision: 2,
                        ..Default::default()
                    },
                ))
            }),
        }
    }

    /// Publish only the actual mounted owner and its existing choice handles.
    /// Tests claim the same production OpLock the owner consults, never a
    /// stand-in boolean or a fixture-side mutation/retirement dispatcher.
    #[derive(Clone, Copy)]
    struct OrdinaryFixture {
        owner: LaunchDraftState,
        choices: ChoiceTargets,
    }

    std::thread_local! {
        static ORDINARY_FIXTURE: std::cell::Cell<Option<OrdinaryFixture>> = const { std::cell::Cell::new(None) };
    }

    /// Mount no reactive draft values so same-turn edits can precede Memo work.
    /// The slot passed to the owner is also the slot the test inspects directly.
    fn ordinary_fixture() -> Element {
        let ops = crate::ops::use_op_lock();
        let key = use_signal(|| None);
        let chosen_host = use_signal(|| None);
        let owner = use_launch_draft(None, None, ops, key, chosen_host);
        let choices = use_choice_targets();
        let source = use_source_targets(choices);
        use_placement_handles(SystemFixture {
            ordinary: OrdinaryFixture { owner, choices },
            source,
        });
        ORDINARY_FIXTURE.with(|slot| slot.set(Some(OrdinaryFixture { owner, choices })));
        rsx! {}
    }

    /// Mount the existing ancillary source handles, with no fixture-side source
    /// model. Choice/history and source code see the same target and hint aliases.
    fn use_source_targets(choices: ChoiceTargets) -> SourceTargets {
        SourceTargets {
            create_target: choices.create_target,
            bound_target: use_signal(|| None),
            remembered_destination: use_signal(|| None),
            prefill_applied: use_signal(|| None),
            mount_seeded: use_signal(|| false),
            partial_template_hint: choices.partial_template_hint,
            auto_launch_pending: use_signal(|| true),
            quick_launch_hidden: use_signal(|| true),
            pending_initial_action: use_signal(|| {
                Some(crate::launch_composer::ComposerSearchResult::Name(
                    "pending".into(),
                ))
            }),
            pending_initial_templates: use_signal(|| {
                std::collections::VecDeque::from([farhelm_proto::launcher::LaunchTemplate {
                    name: "pending".into(),
                    fields: Default::default(),
                }])
            }),
            clone_host_state: use_signal(|| CloneHostState::Unconfirmable),
            preview_revision: use_signal(|| 0),
            browse_generation: use_signal(|| 4),
            browse_request: use_signal(|| None),
            browse_result: use_signal(|| None),
            browse_error: use_signal(|| Some("old browse failure".into())),
            error: use_signal(|| Some("old refusal".into())),
            preparation_error: use_signal(|| Some("old preparation".into())),
            composer_reset_reason: use_signal(|| Some("old reset".into())),
            permission_escalation: use_signal(|| None),
            template_panel: use_signal(|| None),
        }
    }

    /// Observe the actual private aggregate only in tests. A field Memo could
    /// conceal an erroneous equal publication; this effect distinguishes it.
    #[derive(Clone, Copy)]
    struct SystemFixture {
        ordinary: OrdinaryFixture,
        source: SourceTargets,
    }

    std::thread_local! {
        static SYSTEM_FIXTURE: std::cell::Cell<Option<SystemFixture>> = const { std::cell::Cell::new(None) };
        static PUBLICATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    /// Keep observation and mutation on the same real Signal and OpLock. The
    /// counter measures publication, not derived naming/request equality.
    fn use_system_fixture() -> SystemFixture {
        let ops = crate::ops::use_op_lock();
        let key = use_signal(|| None);
        let chosen_host = use_signal(|| None);
        let owner = use_launch_draft(None, None, ops, key, chosen_host);
        let choices = use_choice_targets();
        let source = use_source_targets(choices);
        use_placement_handles(SystemFixture {
            ordinary: OrdinaryFixture { owner, choices },
            source,
        });
        use_effect(move || {
            let _ = owner.value.read();
            PUBLICATIONS.with(|count| count.set(count.get() + 1));
        });
        let fixture = SystemFixture {
            ordinary: OrdinaryFixture { owner, choices },
            source,
        };
        SYSTEM_FIXTURE.with(|slot| slot.set(Some(fixture)));
        fixture
    }

    /// Render no draft fields; process_events can settle only the named effects.
    fn system_fixture() -> Element {
        use_system_fixture();
        rsx! {}
    }

    /// Reuse the system fixture's real source/browse handles for placement.
    /// UI state is deliberately distinguishable so a refused queued callback
    /// cannot appear correct merely because every cleanup already held defaults.
    #[derive(Clone, Copy)]
    struct PlacementFixture {
        system: SystemFixture,
        targets: PlacementTargets,
        repository: RepositoryTargets,
        folder: FolderTargets,
        connection: Signal<Option<u64>>,
        attempts: Signal<u64>,
    }

    std::thread_local! {
        static PLACEMENT_FIXTURE: std::cell::Cell<Option<PlacementFixture>> = const { std::cell::Cell::new(None) };
    }

    /// Mount only production handles; measured transitions call the owner.
    fn use_placement_fixture() -> PlacementFixture {
        use_system_fixture();
        PLACEMENT_FIXTURE
            .with(std::cell::Cell::get)
            .expect("the system hook mounted the shared placement handles")
    }

    /// Existing Memo probes can share these real aliases around their own
    /// mount-time raw cwd without adding another draft or publication oracle.
    fn use_placement_handles(system: SystemFixture) -> PlacementFixture {
        let source = system.source;
        let targets = PlacementTargets {
            choices: system.ordinary.choices,
            live_destination: use_signal(|| Some(CreateTarget::new(1, "install-a".into()))),
            remembered_destination: source.remembered_destination,
            clone_host_state: source.clone_host_state,
            history_activation_attempts: use_signal(|| 0),
            preview_revision: source.preview_revision,
            browse_generation: source.browse_generation,
            browse_request: source.browse_request,
            browse_result: source.browse_result,
            browse_error: source.browse_error,
        };
        let fixture = PlacementFixture {
            system,
            targets,
            repository: RepositoryTargets {
                open: use_signal(|| true),
                index: use_signal(|| 3),
                search: use_signal(|| "gh:pending".into()),
                search_open: use_signal(|| true),
            },
            folder: FolderTargets {
                open: use_signal(|| true),
                active: use_signal(|| Some(2)),
            },
            connection: use_signal(|| Some(4)),
            attempts: use_signal(|| 0),
        };
        use_final_context(targets, source);
        PLACEMENT_FIXTURE.with(|slot| slot.set(Some(fixture)));
        fixture
    }

    /// All final-family fixtures share actual alias handles and preference data.
    /// No helper can publish a supplied draft; it must call a concrete operation.
    #[derive(Clone, Copy)]
    struct FinalContext {
        search: SearchTargets,
        model: ModelTargets,
        source: SourceTargets,
        preferences: SharedPreferences,
        opening: Signal<u64>,
        notice_generation: Signal<u64>,
        retained: Signal<Option<(IntentBinding, crate::github_checkout::GithubAttempt)>>,
    }
    std::thread_local! {
        static FINAL_CONTEXT: std::cell::Cell<Option<FinalContext>> = const { std::cell::Cell::new(None) };
    }

    /// Mount independent picker candidates around the same production owner.
    fn use_final_context(placement: PlacementTargets, source: SourceTargets) {
        let draft = use_signal(String::new);
        let error = use_signal(|| None);
        let recovery = RecoveryTargets {
            composer_reset_reason: source.composer_reset_reason,
            clone_host_state: source.clone_host_state,
            remembered_destination: source.remembered_destination,
            model_draft: draft,
            model_draft_error: error,
        };
        let model = ModelTargets {
            reset_reason: source.composer_reset_reason,
            draft,
            error,
            open: use_signal(|| false),
            active: use_signal(|| None),
        };
        let context = FinalContext {
            search: SearchTargets {
                placement,
                recovery,
                template_error: use_signal(|| None),
            },
            model,
            source,
            preferences: SharedPreferences(use_signal(api::Preferences::default)),
            opening: use_signal(|| 1),
            notice_generation: use_signal(|| 0),
            retained: use_signal(|| None),
        };
        FINAL_CONTEXT.with(|slot| slot.set(Some(context)));
    }

    /// Use production replay, including validation and the full-pair policy.
    fn apply_fields(owner: LaunchDraftState, fields: farhelm_proto::launcher::TemplateFields) {
        let context = FINAL_CONTEXT.with(std::cell::Cell::get).unwrap();
        let template = farhelm_proto::launcher::LaunchTemplate {
            name: "fixture".into(),
            fields,
        };
        let accepted = owner.accept_search_result(
            crate::launch_composer::ComposerSearchResult::Template("fixture".into()),
            Some(&template),
            false,
            SearchInputs {
                templates: &[],
                hosts: &[],
                catalog: Some(&[]),
                host_fixed: false,
                history_target: None,
                preferences: context.preferences,
            },
            context.search,
        );
        assert!(accepted.is_some(), "fixture replay must be admitted");
        assert!(
            context.search.template_error.peek().is_none(),
            "fixture template must apply: {:?}",
            context.search.template_error.peek()
        );
    }

    /// Fixture agent selection earns the real Control-origin admission/capture.
    fn choose_fixture_harness(owner: LaunchDraftState, harness: LaunchHarness) {
        let context = FINAL_CONTEXT.with(std::cell::Cell::get).unwrap();
        assert!(
            owner
                .choose_harness(
                    harness,
                    None,
                    context.preferences,
                    context.search.placement.choices,
                    context.search.recovery
                )
                .is_some()
        );
    }

    /// Occupied setup exercises the same finite authority/default gate as replies.
    fn advance_fixture_name(owner: LaunchDraftState) -> bool {
        let authority = naming_authority(&owner.snapshot());
        owner.advance_checkout_name_for_occupied_reply(
            &authority,
            &authority,
            &api::GithubPreviewFailure {
                message: "occupied".into(),
                occupied: true,
            },
            false,
        )
    }

    /// Seed unsafe bytes through the real source operation. Both retained launch
    /// groups supply the fixture's source facts; ordinary tab/prompt/resume edits
    /// then preserve the observer's values. Other raw ownership may be reseeded,
    /// which must not create an aggregate preview subscription either.
    fn seed_invocation_from_source(owner: LaunchDraftState, raw: &str) {
        let context = FINAL_CONTEXT.with(std::cell::Cell::get).unwrap();
        let before = owner.snapshot();
        let mut source = source_prefill();
        source.generation = context.source.prefill_applied.peek().unwrap_or(0) + 1;
        source.cwd = before.submitted_cwd();
        source.title = before.effective_title();
        source.repo = None;
        source.invocation = raw.into();
        source.launch = before.structured_selection();
        source.command = before.command_yolo().map(|yolo| CommandLaunch {
            command: raw.into(),
            yolo,
            agent: before.command_agent(),
            resume: before.command_resume_on().then(|| {
                submitted_field(
                    &before.command_resume(),
                    before.command_resume_edited(),
                    before.command_resume_raw_seed().as_deref(),
                )
            }),
        });
        owner.reconcile_source(Some(&source), false, &[], context.source);
        // An empty peer title displays as "(empty)". Keep this invocation-only
        // fixture's original blank editor through a real title edit, rather
        // than introducing a name before the later same-turn placement probe.
        if before.title().is_empty() {
            owner.edit_title(String::new());
        }
        owner.choose_launch_tab(before.tab(), context.search.placement.choices);
        if let Some(prompt) = before.prompt() {
            owner.edit_prompt(prompt);
        }
        if before.command_resume_edited() {
            owner.edit_command_resume(before.command_resume());
        }
    }

    /// Recovery must come from the actual completed switch notice, retaining raw
    /// and dormant values while excluding retry/preview authority. Every final
    /// editing entry also rejects under the real same-turn operation claim.
    #[farhelm_testtrace::test]
    fn final_owner_gates_composites_and_restores_current_immutable_undo() {
        let mut dom = VirtualDom::new(placement_fixture);
        dom.rebuild_in_place();
        dom.process_events();
        let f = PLACEMENT_FIXTURE.with(std::cell::Cell::get).unwrap();
        let owner = f.system.ordinary.owner;
        let mut c = FINAL_CONTEXT.with(std::cell::Cell::get).unwrap();
        let mut key = owner.intent_key;
        dom.in_runtime(|| {
            let mut source = source_prefill();
            source.repo = None;
            source.title = "peer-name\u{202e}".into();
            owner.reconcile_source(Some(&source), false, &[], c.source);
            choose_fixture_harness(owner, LaunchHarness::Codex);
            owner.choose_permissions(Some(LaunchPermission::Guarded), f.targets.choices);
            owner.choose_command_agent(Some(LaunchHarness::Claude));
            owner.choose_command_resume(true);
            let mut recent = placement_recent(&source.cwd);
            recent.selection.model = Some("private-model\u{202e}".into());
            recent.selection.permissions = Some(LaunchPermission::Guarded);
            assert!(owner.accept_recent(
                recent,
                &[],
                Some(CreateTarget::new(1, "install-a".into())),
                None,
                f.targets,
                c.source.permission_escalation,
                c.source.composer_reset_reason
            ));
            owner.edit_permission_flags("'unfinished dormant flags".into());
            owner.edit_wrapper("nice 'two words'".into());
            owner.edit_prompt("raw prompt\nsecond".into());
            owner.edit_conversation("conversation".into());
            c.search
                .recovery
                .model_draft
                .set("candidate before capture".into());
            c.search
                .recovery
                .model_draft_error
                .set(Some("candidate error".into()));
            c.search
                .recovery
                .composer_reset_reason
                .set(Some("old reset".into()));
            c.search
                .recovery
                .remembered_destination
                .set(Some(CreateTarget::new(1, "install-a".into())));
            let mut host = owner.chosen_host;
            host.set(Some(1));
            c.search
                .recovery
                .clone_host_state
                .set(CloneHostState::Bound);
            let before = owner.snapshot();
            assert_ne!(before.cwd(), before.submitted_cwd());
            assert!(!before.command_resume_edited());
            assert!(!before.model_edited());
            assert_eq!(
                before.model_raw_seed(),
                Some("private-model\u{202e}".into())
            );
            let pair = Some(("before-switch".into(), request(&before)));
            key.set(pair.clone());
            c.retained.set(Some((
                pair.as_ref().unwrap().1.clone(),
                crate::github_checkout::GithubAttempt::new(
                    "retained".into(),
                    serde_json::json!({"frozen": "body"}),
                    "install-a".into(),
                ),
            )));
            let retained = c.retained.peek().clone();
            let mut ops = owner.ops;
            let guard = ops.claim_guard().unwrap();
            assert!(
                owner
                    .choose_harness(
                        LaunchHarness::Pi,
                        None,
                        c.preferences,
                        f.targets.choices,
                        c.search.recovery
                    )
                    .is_none()
            );
            assert!(
                owner
                    .choose_catalog_model(
                        "other".into(),
                        LaunchHarness::Pi,
                        None,
                        c.preferences,
                        f.targets.choices,
                        c.search.recovery
                    )
                    .is_none()
            );
            assert!(!owner.choose_default_model(f.targets.choices, c.model));
            assert!(!owner.choose_custom_model(
                "custom".into(),
                LaunchHarness::Codex,
                None,
                f.targets.choices,
                c.model
            ));
            assert!(!owner.report_model_harness_required(c.model.error));
            assert!(!owner.reset_choices(
                c.preferences,
                f.targets.choices,
                c.model.reset_reason,
                f.repository.search,
                f.repository.search_open
            ));
            assert!(
                owner
                    .accept_search_result(
                        crate::launch_composer::ComposerSearchResult::Name("blocked".into()),
                        None,
                        false,
                        SearchInputs {
                            templates: &[],
                            hosts: &[],
                            catalog: None,
                            host_fixed: false,
                            history_target: None,
                            preferences: c.preferences
                        },
                        c.search
                    )
                    .is_none()
            );
            let blocked = farhelm_proto::launcher::LaunchTemplate {
                name: "blocked".into(),
                fields: farhelm_proto::launcher::TemplateFields {
                    name: Some("blocked".into()),
                    ..Default::default()
                },
            };
            assert!(
                owner
                    .accept_search_result(
                        crate::launch_composer::ComposerSearchResult::Template("blocked".into()),
                        Some(&blocked),
                        false,
                        SearchInputs {
                            templates: &[],
                            hosts: &[],
                            catalog: None,
                            host_fixed: false,
                            history_target: None,
                            preferences: c.preferences
                        },
                        c.search
                    )
                    .is_none()
            );
            assert_eq!(owner.snapshot(), before);
            assert_eq!(*key.peek(), pair);
            assert_eq!(
                c.search.recovery.model_draft.peek().as_str(),
                "candidate before capture"
            );
            drop(guard);
            let captured = owner
                .choose_harness(
                    LaunchHarness::Pi,
                    None,
                    c.preferences,
                    f.targets.choices,
                    c.search.recovery,
                )
                .unwrap();
            assert_eq!(captured.undo.draft, before);
            assert_eq!(captured.undo.model_draft, "candidate before capture");
            assert_eq!(
                captured.undo.model_draft_error.as_deref(),
                Some("candidate error")
            );
            assert!(
                key.peek().is_none(),
                "settled before the completed observer"
            );
            super::super::note_completed_permission_switch(
                owner,
                captured,
                true,
                c.source.permission_escalation,
                c.notice_generation,
                c.opening,
            );
            let notice = c
                .source
                .permission_escalation
                .peek()
                .clone()
                .expect("real automatic Guarded to Pi increase creates recovery");
            assert_eq!(notice.undo.draft, before);
            assert_eq!(notice.undo.chosen_host, Some(1));
            assert_eq!(notice.undo.clone_host_state, CloneHostState::Bound);
            let after = owner.snapshot();
            key.set(Some(("after-switch".into(), request(&after))));
            let after_pair = key.peek().clone();
            c.opening.set(2);
            assert!(!owner.undo_permission_switch(
                notice.generation,
                c.search.recovery,
                c.source.permission_escalation,
                c.opening,
                f.targets
            ));
            c.opening.set(1);
            assert!(!owner.undo_permission_switch(
                notice.generation + 1,
                c.search.recovery,
                c.source.permission_escalation,
                c.opening,
                f.targets
            ));
            let guard = ops.claim_guard().unwrap();
            assert!(!owner.undo_permission_switch(
                notice.generation,
                c.search.recovery,
                c.source.permission_escalation,
                c.opening,
                f.targets
            ));
            assert_eq!(owner.snapshot(), after);
            assert_eq!(*key.peek(), after_pair);
            drop(guard);
            c.search.recovery.model_draft.set("later candidate".into());
            let mut host = owner.chosen_host;
            host.set(Some(9));
            c.search
                .recovery
                .clone_host_state
                .set(CloneHostState::UserTookOver);
            c.search.recovery.remembered_destination.set(None);
            let live = f.targets.live_destination.peek().clone();
            let browse_generation = *f.targets.browse_generation.peek();
            let revision = *f.targets.preview_revision.peek();
            assert!(owner.undo_permission_switch(
                notice.generation,
                c.search.recovery,
                c.source.permission_escalation,
                c.opening,
                f.targets
            ));
            assert_eq!(
                owner.snapshot(),
                before,
                "Undo restores all raw/dormant fields as one value"
            );
            assert_eq!(*owner.chosen_host.peek(), Some(1));
            assert_eq!(
                *c.search.recovery.clone_host_state.peek(),
                CloneHostState::Bound
            );
            assert_eq!(
                *c.search.recovery.remembered_destination.peek(),
                notice.undo.remembered_destination
            );
            assert_eq!(
                c.search.recovery.model_draft.peek().as_str(),
                "candidate before capture"
            );
            assert_eq!(
                c.search.recovery.model_draft_error.peek().as_deref(),
                Some("candidate error")
            );
            assert_eq!(
                *f.targets.live_destination.peek(),
                live,
                "Undo does not synchronously restore this fence"
            );
            assert_eq!(*f.targets.browse_generation.peek(), browse_generation + 1);
            assert_eq!(*f.targets.preview_revision.peek(), revision + 1);
            assert!(key.peek().is_none());
            assert!(c.source.permission_escalation.peek().is_none());
            assert_eq!(*c.retained.peek(), retained);
            assert!(!owner.undo_permission_switch(
                notice.generation,
                c.search.recovery,
                c.source.permission_escalation,
                c.opening,
                f.targets
            ));
            let captured = owner
                .choose_catalog_model(
                    "pi-model".into(),
                    LaunchHarness::Pi,
                    None,
                    c.preferences,
                    f.targets.choices,
                    c.search.recovery,
                )
                .unwrap();
            assert_eq!(captured.undo.model_draft, "candidate before capture");
            super::super::note_completed_permission_switch(
                owner,
                captured,
                true,
                c.source.permission_escalation,
                c.notice_generation,
                c.opening,
            );
            c.model.complete();
            let catalog_notice = c
                .source
                .permission_escalation
                .peek()
                .clone()
                .expect("cross-agent catalog pick observes completed increase");
            assert_eq!(
                catalog_notice.undo.model_draft_error.as_deref(),
                Some("candidate error")
            );
            assert!(owner.undo_permission_switch(
                catalog_notice.generation,
                c.search.recovery,
                c.source.permission_escalation,
                c.opening,
                f.targets
            ));
            // Equal model/default cleanup and NeedsHarness are different fixed
            // operations: only the latter eagerly ends an unchanged pair.
            owner.choose_default_model(f.targets.choices, c.model);
            let pair = Some(("default".into(), request(&owner.snapshot())));
            key.set(pair.clone());
            c.model.draft.set("discard".into());
            c.model.error.set(Some("discard".into()));
            c.model.open.set(true);
            assert!(owner.choose_default_model(f.targets.choices, c.model));
            assert_eq!(*key.peek(), pair);
            assert_eq!(c.model.draft.peek().as_str(), "");
            assert!(c.model.error.peek().is_none() && !*c.model.open.peek());
            assert!(owner.choose_custom_model(
                "same-custom".into(),
                LaunchHarness::Codex,
                None,
                f.targets.choices,
                c.model
            ));
            let custom_pair = Some(("custom".into(), request(&owner.snapshot())));
            key.set(custom_pair.clone());
            c.model.draft.set("discard equal custom".into());
            assert!(owner.choose_custom_model(
                "same-custom".into(),
                LaunchHarness::Codex,
                None,
                f.targets.choices,
                c.model
            ));
            assert_eq!(*key.peek(), custom_pair);
            assert_eq!(c.model.draft.peek().as_str(), "");
            assert!(owner.report_model_harness_required(c.model.error));
            assert!(key.peek().is_none());
            c.preferences
                .0
                .with_mut(|p| p.remembered_permissions = Some("chat".into()));
            let command = owner.snapshot().invocation();
            let prompt = owner.snapshot().prompt();
            assert!(owner.reset_choices(
                c.preferences,
                f.targets.choices,
                c.model.reset_reason,
                f.repository.search,
                f.repository.search_open
            ));
            assert_eq!(owner.snapshot().permissions(), Some(LaunchPermission::Chat));
            assert_eq!(owner.snapshot().permission_flags(), "");
            assert_eq!(owner.snapshot().invocation(), command);
            assert_eq!(owner.snapshot().prompt(), prompt);
            assert_eq!(owner.snapshot().conversation(), "conversation");
        });
    }

    /// Whole templates preserve even mismatched pairs; validation refuses before
    /// semantic publication while retaining its admitted browse/history preamble.
    /// Search BrowsePath equality must still request the raw directory afterward.
    #[farhelm_testtrace::test]
    fn final_owner_replays_templates_with_fixed_pair_and_partial_prompt_contracts() {
        let mut dom = VirtualDom::new(placement_fixture);
        dom.rebuild_in_place();
        dom.process_events();
        let f = PLACEMENT_FIXTURE.with(std::cell::Cell::get).unwrap();
        let owner = f.system.ordinary.owner;
        let c = FINAL_CONTEXT.with(std::cell::Cell::get).unwrap();
        let mut key = owner.intent_key;
        dom.in_runtime(|| {
            apply_fields(
                owner,
                farhelm_proto::launcher::TemplateFields {
                    prompt: Some(String::new()),
                    ..Default::default()
                },
            );
            assert_eq!(owner.snapshot().harness(), None);
            assert_eq!(owner.snapshot().prompt(), Some(String::new()));
            apply_fields(
                owner,
                farhelm_proto::launcher::TemplateFields {
                    agent: Some(LaunchHarness::Codex),
                    ..Default::default()
                },
            );
            assert_eq!(
                request(&owner.snapshot()).prompt,
                Some(String::new()),
                "a later template supplies the agent without losing empty presence"
            );
            let pair = Some(("deliberately-old".into(), request(&owner.snapshot())));
            key.set(pair.clone());
            apply_fields(
                owner,
                farhelm_proto::launcher::TemplateFields {
                    name: Some("changed".into()),
                    ..Default::default()
                },
            );
            assert_eq!(
                *key.peek(),
                pair,
                "changed completed template keeps mismatched old binding"
            );
            apply_fields(
                owner,
                farhelm_proto::launcher::TemplateFields {
                    name: Some("".into()),
                    ..Default::default()
                },
            );
            assert_eq!(*key.peek(), pair);
            apply_fields(
                owner,
                farhelm_proto::launcher::TemplateFields {
                    prompt: Some(String::new()),
                    ..Default::default()
                },
            );
            assert_eq!(owner.snapshot().prompt(), Some(String::new()));
            assert_eq!(request(&owner.snapshot()).prompt, Some(String::new()));
            let before = owner.snapshot();
            let generation = *f.targets.browse_generation.peek();
            let template = farhelm_proto::launcher::LaunchTemplate {
                name: "invalid".into(),
                fields: farhelm_proto::launcher::TemplateFields {
                    kind: Some(farhelm_proto::launcher::LauncherKind::Command),
                    model: Some(Some("inapplicable".into())),
                    ..Default::default()
                },
            };
            let accepted = owner
                .accept_search_result(
                    crate::launch_composer::ComposerSearchResult::Template("invalid".into()),
                    Some(&template),
                    false,
                    SearchInputs {
                        templates: &[],
                        hosts: &[],
                        catalog: Some(&[]),
                        host_fixed: false,
                        history_target: None,
                        preferences: c.preferences,
                    },
                    c.search,
                )
                .unwrap();
            assert!(accepted.is_template);
            assert!(c.search.template_error.peek().is_some());
            assert_eq!(owner.snapshot(), before);
            assert_eq!(*key.peek(), pair);
            assert_eq!(*f.targets.browse_generation.peek(), generation + 1);
            let path = owner.snapshot().submitted_cwd();
            let accepted = owner
                .accept_search_result(
                    crate::launch_composer::ComposerSearchResult::BrowsePath(path.clone()),
                    None,
                    false,
                    SearchInputs {
                        templates: &[],
                        hosts: &[],
                        catalog: None,
                        host_fixed: false,
                        history_target: None,
                        preferences: c.preferences,
                    },
                    c.search,
                )
                .unwrap();
            assert_eq!(accepted.browse_path, Some(path));
            assert_eq!(*key.peek(), pair);
            let before = owner.snapshot();
            let revision = *f.targets.preview_revision.peek();
            apply_fields(
                owner,
                farhelm_proto::launcher::TemplateFields {
                    name: Some("base".into()),
                    name_kind: Some(farhelm_proto::launcher::TemplateNameKind::Fixed),
                    ..Default::default()
                },
            );
            apply_fields(
                owner,
                farhelm_proto::launcher::TemplateFields {
                    name: Some("base".into()),
                    name_kind: Some(farhelm_proto::launcher::TemplateNameKind::Base),
                    ..Default::default()
                },
            );
            assert_eq!(*f.targets.preview_revision.peek(), revision + 2);
            assert_eq!(*key.peek(), pair);
            assert_eq!(owner.snapshot().prompt(), before.prompt());
            let explicit = farhelm_proto::launcher::LaunchTemplate {
                name: "explicit".into(),
                fields: farhelm_proto::launcher::TemplateFields {
                    agent: Some(LaunchHarness::Pi),
                    permissions: Some(None),
                    ..Default::default()
                },
            };
            let accepted = owner
                .accept_search_result(
                    crate::launch_composer::ComposerSearchResult::Template("explicit".into()),
                    Some(&explicit),
                    false,
                    SearchInputs {
                        templates: &[],
                        hosts: &[],
                        catalog: Some(&[]),
                        host_fixed: false,
                        history_target: None,
                        preferences: c.preferences,
                    },
                    c.search,
                )
                .unwrap();
            assert!(c.search.template_error.peek().is_none());
            assert!(!accepted.automatic_permission);
            super::super::note_completed_permission_switch(
                owner,
                accepted.before,
                accepted.automatic_permission,
                c.source.permission_escalation,
                c.notice_generation,
                c.opening,
            );
            assert!(
                c.source.permission_escalation.peek().is_none(),
                "explicit final permission suppresses the intermediate automatic increase"
            );
            choose_fixture_harness(owner, LaunchHarness::Codex);
            owner.edit_prompt("restore prompt".into());
            let automatic = farhelm_proto::launcher::LaunchTemplate {
                name: "automatic".into(),
                fields: farhelm_proto::launcher::TemplateFields {
                    agent: Some(LaunchHarness::Pi),
                    ..Default::default()
                },
            };
            key.set(pair.clone());
            let accepted = owner
                .accept_search_result(
                    crate::launch_composer::ComposerSearchResult::Template("automatic".into()),
                    Some(&automatic),
                    false,
                    SearchInputs {
                        templates: &[],
                        hosts: &[],
                        catalog: Some(&[]),
                        host_fixed: false,
                        history_target: None,
                        preferences: c.preferences,
                    },
                    c.search,
                )
                .unwrap();
            assert!(accepted.automatic_permission);
            assert_eq!(*key.peek(), pair);
            super::super::note_completed_permission_switch(
                owner,
                accepted.before,
                accepted.automatic_permission,
                c.source.permission_escalation,
                c.notice_generation,
                c.opening,
            );
            assert!(
                c.source.permission_escalation.peek().is_some(),
                "automatic final increase has the real positive notice control"
            );
        });
    }

    /// Initial replay earns the original seven conditional field dependencies.
    /// A real Harness acceptance establishes them; each named edit is a positive
    /// control, while prompt/name edits prove no aggregate subscription leaked.
    #[farhelm_testtrace::test]
    fn final_owner_search_keeps_conditional_agent_subscriptions() {
        std::thread_local! { static RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
        /// Constant captured input isolates dependencies earned by actual replay.
        fn probe() -> Element {
            let f = use_placement_fixture();
            let c = FINAL_CONTEXT.with(std::cell::Cell::get).unwrap();
            use_effect(move || {
                RUNS.with(|count| count.set(count.get() + 1));
                f.system
                    .ordinary
                    .owner
                    .accept_search_result(
                        crate::launch_composer::ComposerSearchResult::Harness(LaunchHarness::Codex),
                        None,
                        false,
                        SearchInputs {
                            templates: &[],
                            hosts: &[],
                            catalog: None,
                            host_fixed: false,
                            history_target: None,
                            preferences: c.preferences,
                        },
                        c.search,
                    )
                    .unwrap();
                f.system
                    .ordinary
                    .owner
                    .accept_search_result(
                        crate::launch_composer::ComposerSearchResult::Model {
                            id: "fixture-model".into(),
                            harness: LaunchHarness::Codex,
                        },
                        None,
                        false,
                        SearchInputs {
                            templates: &[],
                            hosts: &[],
                            catalog: None,
                            host_fixed: false,
                            history_target: None,
                            preferences: c.preferences,
                        },
                        c.search,
                    )
                    .unwrap();
            });
            rsx! {}
        }
        RUNS.with(|count| count.set(0));
        let mut dom = VirtualDom::new(probe);
        dom.rebuild_in_place();
        dom.process_events();
        let f = PLACEMENT_FIXTURE.with(std::cell::Cell::get).unwrap();
        let owner = f.system.ordinary.owner;
        let c = FINAL_CONTEXT.with(std::cell::Cell::get).unwrap();
        let initial = RUNS.with(std::cell::Cell::get);
        assert!(initial >= 1);
        assert_eq!(
            dom.in_runtime(|| owner.snapshot().harness()),
            Some(LaunchHarness::Codex)
        );
        dom.in_runtime(|| {
            owner.edit_prompt("unrelated".into());
            owner.edit_title("unrelated".into());
        });
        dom.process_events();
        assert_eq!(RUNS.with(std::cell::Cell::get), initial);
        let edits: [fn(LaunchDraftState, FinalContext); 7] = [
            |o, c| {
                o.choose_permissions(Some(LaunchPermission::Custom), c.search.placement.choices);
            },
            |o, _| {
                o.edit_permission_flags("--other".into());
            },
            |o, _| {
                o.edit_wrapper("wrapper".into());
            },
            |o, c| {
                o.choose_harness(
                    LaunchHarness::Claude,
                    None,
                    c.preferences,
                    c.search.placement.choices,
                    c.search.recovery,
                );
            },
            |o, c| {
                o.choose_custom_model(
                    "private".into(),
                    LaunchHarness::Codex,
                    None,
                    c.search.placement.choices,
                    c.model,
                );
            },
            |o, c| {
                o.choose_effort(Some(LaunchEffort::High), c.search.placement.choices);
            },
            |o, c| {
                o.reset_choices(
                    c.preferences,
                    c.search.placement.choices,
                    c.model.reset_reason,
                    Signal::new(String::new()),
                    Signal::new(false),
                );
            },
        ];
        for edit in edits {
            let before = RUNS.with(std::cell::Cell::get);
            dom.in_scope(ScopeId::ROOT, || edit(owner, c));
            dom.process_events();
            assert!(
                RUNS.with(std::cell::Cell::get) > before,
                "named field remains a replay dependency"
            );
        }
        dom.in_runtime(|| {
            let before = owner.snapshot();
            assert!(!before.permissions_explicit());
            assert_eq!(before.permissions(), Some(LaunchPermission::Guarded));
            // Search has already reconciled Codex's passive Guarded default.
            // Reaccept that exact value so only explicitness can wake replay.
            owner.choose_permissions(before.permissions(), c.search.placement.choices);
            let mut expected = before;
            expected.permissions_explicit = true;
            assert_eq!(owner.snapshot(), expected, "only explicitness changed");
        });
        let before = RUNS.with(std::cell::Cell::get);
        dom.process_events();
        assert!(
            RUNS.with(std::cell::Cell::get) > before,
            "explicitness alone stays subscribed"
        );
    }

    /// No render subscription can substitute for the operation's live gate.
    fn placement_fixture() -> Element {
        use_placement_fixture();
        rsx! {}
    }

    /// Saved setup bytes come from the real plain selection projection.
    fn placement_recent(cwd: &str) -> api::LaunchHistoryEntry {
        let mut plain = LaunchDraft::new(None);
        plain.choose_harness(
            LaunchHarness::Codex,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        api::LaunchHistoryEntry {
            host: 1,
            github_repo: None,
            canonical_cwd: None,
            cwd: cwd.into(),
            selection: plain.structured_selection().unwrap(),
            created_at: 1,
            creation_seq: Some(1),
        }
    }

    /// Establish an executable Command draft through existing concrete edits.
    /// A subsequent Recent must retain this dormant group and one-time inputs.
    fn seed_placement_command(fixture: PlacementFixture) {
        let owner = fixture.system.ordinary.owner;
        assert!(owner.choose_launch_tab(LaunchTab::Command, fixture.targets.choices));
        assert!(owner.edit_invocation("run-command".into()));
        assert!(owner.choose_command_yolo(true));
        assert!(owner.edit_prompt("keep prompt".into()));
        assert!(owner.edit_conversation("keep conversation".into()));
    }

    /// Live locking precedes every new operation's reads and ancillary writes.
    /// Browse alone counts entry before refusal. Same-mode and rendered read-only
    /// refusals then prove domain rejection is distinct from an equal edit.
    #[farhelm_testtrace::test]
    fn placement_owner_gates_all_entries_and_keeps_domain_refusals_inert() {
        PUBLICATIONS.with(|count| count.set(0));
        let mut dom = VirtualDom::new(placement_fixture);
        dom.rebuild_in_place();
        dom.process_events();
        let mut f = PLACEMENT_FIXTURE.with(std::cell::Cell::get).unwrap();
        let owner = f.system.ordinary.owner;
        let mut key = owner.intent_key;
        let target = Some(CreateTarget::new(1, "install-a".into()));
        let authority = BrowseAuthority {
            target: target.clone().unwrap(),
            connection: Some(4),
            cwd: "~".into(),
            generation: 4,
        };
        let repo = GithubRepo::parse("acme/repo").unwrap();
        let entry = placement_recent("~");
        dom.in_runtime(|| {
            seed_placement_command(f);
            f.targets.choices.partial_template_hint.set(true);
            f.targets.remembered_destination.set(target.clone());
            f.targets.browse_request.set(Some(authority.clone()));
            key.set(Some((
                "locked-placement".into(),
                request(&owner.snapshot()),
            )));
        });
        dom.process_events();
        let before = dom.in_runtime(|| owner.snapshot());
        let before_publications = PUBLICATIONS.with(std::cell::Cell::get);
        let pair = dom.in_runtime(|| key.peek().clone());
        let offered = dom.in_runtime(|| f.targets.choices.offered_history.peek().clone());
        let mut ops = owner.ops;
        let guard = dom.in_runtime(|| ops.claim_guard().expect("real claim before rerender"));
        dom.in_runtime(|| {
            assert!(!owner.choose_host(1, &[], f.targets));
            assert!(!owner.select_repository(repo.clone(), f.targets, f.repository));
            assert!(!owner.choose_destination_type(true, f.targets, f.repository.open));
            assert!(!owner.edit_repository_query(
                "new/repo".into(),
                false,
                f.targets,
                f.repository
            ));
            assert!(!owner.clear_repository(f.targets, f.repository));
            assert!(!owner.edit_folder("/changed".into(), f.targets, f.folder));
            assert!(!owner.choose_folder(
                FolderChoice::Home,
                Some(1),
                target.clone(),
                f.targets,
                f.folder
            ));
            assert!(!owner.accept_recent(
                entry.clone(),
                &[],
                target.clone(),
                offered.clone(),
                f.targets,
                f.system.source.permission_escalation,
                f.system.source.composer_reset_reason
            ));
            assert!(!owner.accept_browse_directory(
                "/changed",
                &authority,
                f.targets,
                f.connection,
                f.attempts
            ));
            assert_eq!(owner.snapshot(), before);
            assert_eq!(*key.peek(), pair);
            assert_eq!(*owner.chosen_host.peek(), None);
            assert_eq!(*f.targets.live_destination.peek(), target);
            assert_eq!(
                *f.targets.clone_host_state.peek(),
                CloneHostState::Unconfirmable
            );
            assert_eq!(*f.targets.remembered_destination.peek(), target);
            assert_eq!(*f.targets.browse_request.peek(), Some(authority.clone()));
            assert_eq!(*f.targets.browse_generation.peek(), 4);
            assert_eq!(*f.targets.history_activation_attempts.peek(), 0);
            assert_eq!(*f.attempts.peek(), 1);
            assert_eq!(*f.targets.choices.offered_history.peek(), offered);
            assert!(*f.targets.choices.partial_template_hint.peek());
            assert!(*f.folder.open.peek());
            assert_eq!(*f.folder.active.peek(), Some(2));
            assert!(*f.repository.open.peek());
            assert_eq!(*f.repository.index.peek(), 3);
            assert_eq!(*f.repository.search.peek(), "gh:pending");
            assert!(*f.repository.search_open.peek());
            assert_eq!(*f.targets.preview_revision.peek(), 0);
        });
        dom.process_events();
        assert_eq!(
            PUBLICATIONS.with(std::cell::Cell::get),
            before_publications,
            "locked operations never publish"
        );
        dom.in_runtime(|| drop(guard));
        dom.in_runtime(|| {
            assert!(!owner.choose_destination_type(false, f.targets, f.repository.open));
            assert!(!owner.edit_repository_query("refused".into(), true, f.targets, f.repository));
            assert_eq!(owner.snapshot(), before);
            assert_eq!(*key.peek(), pair);
            assert_eq!(*f.targets.browse_request.peek(), Some(authority));
            assert!(*f.targets.choices.partial_template_hint.peek());
        });
        dom.process_events();
        assert_eq!(
            PUBLICATIONS.with(std::cell::Cell::get),
            before_publications,
            "domain refusals never publish"
        );
        dom.in_runtime(|| {
            assert!(owner.choose_host(1, &[], f.targets));
            assert_eq!(*owner.chosen_host.peek(), Some(1));
            assert!(key.peek().is_none());
            key.set(pair.clone());
            assert!(owner.choose_host(1, &[], f.targets));
            assert!(key.peek().is_none(), "same host is explicit retirement");
            key.set(pair.clone());
            assert!(owner.select_repository(repo.clone(), f.targets, f.repository));
            assert_eq!(owner.snapshot().destination().repo(), Some(&repo));
            assert!(
                owner.fields().destination_draft.peek().repo().is_none(),
                "memo has not settled in this event"
            );
            assert!(key.peek().is_none());
            assert_eq!(*f.targets.preview_revision.peek(), 1);
            key.set(pair.clone());
            assert!(owner.select_repository(repo, f.targets, f.repository));
            assert!(key.peek().is_none());
            assert_eq!(*f.targets.preview_revision.peek(), 2);
            assert!(!*f.repository.open.peek());
            assert_eq!(*f.repository.search.peek(), "");
            assert!(!owner.edit_folder("/forbidden".into(), f.targets, f.folder));
            assert!(!owner.choose_folder(FolderChoice::Home, None, None, f.targets, f.folder));
            key.set(pair.clone());
            assert!(owner.edit_repository_query(
                "new/query".into(),
                false,
                f.targets,
                f.repository
            ));
            assert!(key.peek().is_none());
            assert_eq!(
                owner.snapshot().destination().repository_text(),
                "new/query"
            );
            assert!(*f.repository.open.peek());
            assert_eq!(*f.repository.index.peek(), 0);
            key.set(pair.clone());
            assert!(owner.clear_repository(f.targets, f.repository));
            assert!(key.peek().is_none());
            assert_eq!(owner.snapshot().destination().repository_text(), "");
            assert!(!*f.repository.open.peek());
            key.set(pair.clone());
            assert!(owner.clear_repository(f.targets, f.repository));
            assert!(
                key.peek().is_none(),
                "clearing the same empty proposal remains explicit"
            );
            key.set(pair.clone());
            assert!(owner.choose_destination_type(false, f.targets, f.repository.open));
            assert!(key.peek().is_none());
            assert_eq!(owner.snapshot().submitted_cwd(), "~");
            let dormant = owner.snapshot();
            key.set(pair.clone());
            assert!(owner.choose_destination_type(true, f.targets, f.repository.open));
            assert!(key.peek().is_none());
            assert!(owner.snapshot().destination().is_managed());
            assert_eq!(owner.snapshot().cwd_raw_seed(), dormant.cwd_raw_seed());
            assert!(owner.choose_destination_type(false, f.targets, f.repository.open));
            assert_eq!(command(&owner.snapshot()).command, "run-command");
            assert_eq!(owner.snapshot().prompt().as_deref(), Some("keep prompt"));
        });
        PLACEMENT_FIXTURE.with(|slot| slot.set(None));
    }

    /// Ordinary equality preserves the complete pair without skipping ownership,
    /// UI or history work. Invalid remembered provenance and explicit LocalHome
    /// are separate reasons to retire even when the raw path remains identical.
    #[farhelm_testtrace::test]
    fn placement_owner_preserves_equal_paths_and_direct_recents() {
        let mut dom = VirtualDom::new(placement_fixture);
        dom.rebuild_in_place();
        dom.process_events();
        let mut f = PLACEMENT_FIXTURE.with(std::cell::Cell::get).unwrap();
        let owner = f.system.ordinary.owner;
        let mut key = owner.intent_key;
        let target = Some(CreateTarget::new(1, "install-a".into()));
        dom.in_runtime(|| {
            seed_placement_command(f);
            let mut prefill = source_prefill();
            prefill.mode = LauncherMode::Replace {
                source: "source".into(),
                opened: (crate::SessionStatus::Unknown, 0),
            };
            owner.reconcile_source(Some(&prefill), false, &[], f.system.source);
            owner.edit_prompt("keep prompt".into());
            let raw = prefill.cwd.clone();
            let pair = Some(("raw-path".into(), request(&owner.snapshot())));
            key.set(pair.clone());
            assert_ne!(
                owner.snapshot().cwd(),
                raw,
                "unsafe peer path has distinct display bytes"
            );
            assert!(owner.choose_folder(
                FolderChoice::Recent {
                    cwd: raw.clone(),
                    target: target.clone()
                },
                None,
                None,
                f.targets,
                f.folder
            ));
            assert_eq!(*key.peek(), pair);
            assert_eq!(*f.targets.remembered_destination.peek(), target);
            assert_eq!(owner.snapshot().submitted_cwd(), raw);
            assert!(!owner.snapshot().cwd_edited());
            assert!(!*f.folder.open.peek());
            assert_eq!(*f.folder.active.peek(), None);
            assert_eq!(*f.targets.history_activation_attempts.peek(), 1);
            assert!(owner.choose_folder(FolderChoice::Home, None, None, f.targets, f.folder));
            assert!(key.peek().is_none(), "changed raw path retires");
            let pair = Some(("home".into(), request(&owner.snapshot())));
            key.set(pair.clone());
            assert!(owner.choose_folder(FolderChoice::Home, None, None, f.targets, f.folder));
            assert_eq!(*key.peek(), pair);
            assert!(owner.edit_folder("~".into(), f.targets, f.folder));
            assert_eq!(*key.peek(), pair);
            assert!(owner.snapshot().cwd_edited());
            assert!(*f.folder.open.peek());
            assert_eq!(*f.folder.active.peek(), None);
            f.targets
                .remembered_destination
                .set(Some(CreateTarget::new(2, "predecessor".into())));
            assert!(owner.edit_folder("~".into(), f.targets, f.folder));
            assert!(
                key.peek().is_none(),
                "equal bytes repair invalid provenance"
            );
            key.set(pair.clone());
            assert!(owner.choose_folder(
                FolderChoice::LocalHome,
                Some(1),
                target.clone(),
                f.targets,
                f.folder
            ));
            assert!(key.peek().is_none());
            assert_eq!(*owner.chosen_host.peek(), Some(1));
            assert_eq!(
                *f.targets.clone_host_state.peek(),
                CloneHostState::UserTookOver
            );
            let entry = placement_recent("~");
            // First application changes the active kind. The second is the
            // equality case, still admitting Enter and the captured history.
            assert!(owner.accept_recent(
                entry.clone(),
                &[],
                target.clone(),
                None,
                f.targets,
                f.system.source.permission_escalation,
                f.system.source.composer_reset_reason
            ));
            assert_eq!(owner.snapshot().tab(), LaunchTab::Agent);
            let pair = Some(("equal-recent".into(), request(&owner.snapshot())));
            key.set(pair.clone());
            let captured = Some((
                target.clone().unwrap(),
                api::LaunchHistory {
                    checkout_config_revision: 71,
                    ..Default::default()
                },
            ));
            let before_counter = *f.targets.history_activation_attempts.peek();
            assert!(owner.accept_recent(
                entry.clone(),
                &[],
                target.clone(),
                captured.clone(),
                f.targets,
                f.system.source.permission_escalation,
                f.system.source.composer_reset_reason
            ));
            assert_eq!(*key.peek(), pair);
            assert_eq!(
                *f.targets.history_activation_attempts.peek(),
                before_counter + 1
            );
            assert_eq!(
                *f.targets.choices.offered_history.peek(),
                captured,
                "direct Recent promotes its captured history"
            );
            assert_eq!(
                owner.snapshot().invocation_raw_seed().as_deref(),
                Some("source-command\u{1b}")
            );
            assert_eq!(owner.snapshot().prompt().as_deref(), Some("keep prompt"));
            assert_eq!(owner.snapshot().conversation(), "keep conversation");
            f.targets
                .remembered_destination
                .set(Some(CreateTarget::new(2, "old-install".into())));
            assert!(owner.accept_recent(
                entry.clone(),
                &[],
                target.clone(),
                None,
                f.targets,
                f.system.source.permission_escalation,
                f.system.source.composer_reset_reason
            ));
            assert!(
                key.peek().is_none(),
                "invalid claim is captured before Recent adopts the current target"
            );
            key.set(pair.clone());
            let before = owner.snapshot();
            assert!(!owner.accept_recent(
                entry,
                &[],
                Some(CreateTarget::new(2, "stale".into())),
                None,
                f.targets,
                f.system.source.permission_escalation,
                f.system.source.composer_reset_reason
            ));
            assert_eq!(owner.snapshot(), before);
            assert_eq!(*key.peek(), pair);
            assert_eq!(
                *f.targets.history_activation_attempts.peek(),
                before_counter + 3
            );
            assert!(!owner.choose_folder(
                FolderChoice::Recent {
                    cwd: "/stale".into(),
                    target: None
                },
                None,
                None,
                f.targets,
                f.folder
            ));
            assert_eq!(owner.snapshot(), before);
            assert_eq!(*key.peek(), pair);
            let mut repository_recent = placement_recent("/old-allocation");
            let repo = GithubRepo::parse("acme/repo").unwrap();
            repository_recent.github_repo = Some(repo.clone());
            let revision = *f.targets.preview_revision.peek();
            assert!(owner.accept_recent(
                repository_recent,
                &[],
                target,
                None,
                f.targets,
                f.system.source.permission_escalation,
                f.system.source.composer_reset_reason
            ));
            assert_eq!(owner.snapshot().destination().repo(), Some(&repo));
            assert!(key.peek().is_none());
            assert_eq!(*f.targets.preview_revision.peek(), revision + 1);
            assert!(f.targets.remembered_destination.peek().is_none());
        });
        PLACEMENT_FIXTURE.with(|slot| slot.set(None));
    }

    /// Browse admission binds request generation, installation, connection and
    /// raw cwd independently. Each negative changes one fact; equal and changed
    /// accepted paths prove the guard is active and preserve their distinct keys.
    #[farhelm_testtrace::test]
    fn placement_owner_rechecks_browse_authority_and_live_raw_bytes() {
        let mut dom = VirtualDom::new(placement_fixture);
        dom.rebuild_in_place();
        dom.process_events();
        let mut f = PLACEMENT_FIXTURE.with(std::cell::Cell::get).unwrap();
        let owner = f.system.ordinary.owner;
        let mut key = owner.intent_key;
        let target = CreateTarget::new(1, "install-a".into());
        dom.in_runtime(|| {
            seed_placement_command(f);
            let mut prefill = source_prefill();
            prefill.mode = LauncherMode::Replace {
                source: "source".into(),
                opened: (crate::SessionStatus::Unknown, 0),
            };
            owner.reconcile_source(Some(&prefill), false, &[], f.system.source);
            let authority = BrowseAuthority {
                target: target.clone(),
                connection: Some(4),
                cwd: prefill.cwd.clone(),
                generation: 9,
            };
            f.targets.browse_request.set(Some(authority.clone()));
            let pair = Some(("browse".into(), request(&owner.snapshot())));
            key.set(pair.clone());
            let before = owner.snapshot();
            let offered = f.targets.choices.offered_history.peek().clone();
            assert!(
                browse_activation_is_current(
                    f.targets.browse_request,
                    &authority,
                    f.targets.choices.create_target,
                    f.connection,
                    &before
                ),
                "all authority facts initially match"
            );
            let mut stale = authority.clone();
            stale.generation -= 1;
            assert!(!owner.accept_browse_directory(
                "/stale",
                &stale,
                f.targets,
                f.connection,
                f.attempts
            ));
            f.connection.set(Some(5));
            assert!(!owner.accept_browse_directory(
                "/stale",
                &authority,
                f.targets,
                f.connection,
                f.attempts
            ));
            f.connection.set(Some(4));
            f.targets
                .choices
                .create_target
                .set(Some(CreateTarget::new(2, "successor".into())));
            assert!(!owner.accept_browse_directory(
                "/stale",
                &authority,
                f.targets,
                f.connection,
                f.attempts
            ));
            f.targets.choices.create_target.set(Some(target.clone()));
            assert_eq!(owner.snapshot(), before);
            assert_eq!(*key.peek(), pair);
            assert_eq!(*f.targets.choices.offered_history.peek(), offered);
            assert_eq!(*f.attempts.peek(), 3);
            assert!(owner.accept_browse_directory(
                &prefill.cwd,
                &authority,
                f.targets,
                f.connection,
                f.attempts
            ));
            assert_eq!(*key.peek(), pair, "equal raw path retains full pair");
            assert!(owner.snapshot().folder_is_explicit());
            assert!(!owner.snapshot().cwd_edited());
            assert!(f.targets.browse_request.peek().is_none());
            f.targets
                .remembered_destination
                .set(Some(CreateTarget::new(2, "old-install".into())));
            f.targets.browse_request.set(Some(authority.clone()));
            assert!(owner.accept_browse_directory(
                &prefill.cwd,
                &authority,
                f.targets,
                f.connection,
                f.attempts
            ));
            assert!(
                key.peek().is_none(),
                "equal browse corrects invalid inherited provenance"
            );
            // Typing the escaped spelling changes operational bytes even when
            // the display remains identical in this same renderer turn.
            f.targets.browse_request.set(Some(authority.clone()));
            assert!(owner.edit_folder(owner.snapshot().cwd(), f.targets, f.folder));
            let literal = owner.snapshot();
            assert_eq!(literal.cwd(), before.cwd());
            assert_ne!(literal.submitted_cwd(), authority.cwd);
            f.targets.browse_request.set(Some(authority.clone()));
            let literal_pair = Some(("literal".into(), request(&literal)));
            key.set(literal_pair.clone());
            assert!(!owner.accept_browse_directory(
                "/stale",
                &authority,
                f.targets,
                f.connection,
                f.attempts
            ));
            assert_eq!(owner.snapshot(), literal);
            assert_eq!(*key.peek(), literal_pair);
            let current = BrowseAuthority {
                cwd: literal.submitted_cwd(),
                generation: 10,
                ..authority
            };
            f.targets.browse_request.set(Some(current.clone()));
            assert!(owner.accept_browse_directory(
                "/new",
                &current,
                f.targets,
                f.connection,
                f.attempts
            ));
            assert_eq!(owner.snapshot().submitted_cwd(), "/new");
            assert!(key.peek().is_none());
        });
        PLACEMENT_FIXTURE.with(|slot| slot.set(None));
    }

    /// Placement earns the same tracked history reads only after live admission.
    /// Positive target/fetched changes rerun this real operation; unrelated draft
    /// and claim changes remain peeks and cannot wake the effect.
    #[farhelm_testtrace::test]
    fn placement_owner_preserves_deliberate_history_subscriptions() {
        std::thread_local! {
            static RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        }
        /// This deliberately invokes the actual edit from an effect to observe
        /// its subscriptions, as opening/replay callers can do synchronously.
        fn probe() -> Element {
            let f = use_placement_fixture();
            use_effect(move || {
                RUNS.with(|count| count.set(count.get() + 1));
                assert!(
                    f.system
                        .ordinary
                        .owner
                        .edit_folder("~".into(), f.targets, f.folder)
                );
            });
            rsx! {}
        }
        RUNS.with(|count| count.set(0));
        let mut dom = VirtualDom::new(probe);
        dom.rebuild_in_place();
        dom.process_events();
        let mut f = PLACEMENT_FIXTURE.with(std::cell::Cell::get).unwrap();
        assert_eq!(
            RUNS.with(std::cell::Cell::get),
            1,
            "initial deliberate choice subscribed"
        );
        dom.in_runtime(|| {
            f.system.ordinary.owner.edit_prompt("unrelated".into());
            f.targets
                .live_destination
                .set(Some(CreateTarget::new(2, "new-install".into())));
        });
        dom.process_events();
        assert_eq!(RUNS.with(std::cell::Cell::get), 1);
        dom.in_runtime(|| {
            f.targets
                .choices
                .create_target
                .set(Some(CreateTarget::new(2, "new-install".into())))
        });
        dom.process_events();
        assert_eq!(RUNS.with(std::cell::Cell::get), 2, "target remains tracked");
        dom.in_runtime(|| {
            f.targets.choices.fetched_history.set(Some((
                CreateTarget::new(2, "new-install".into()),
                api::LaunchHistory {
                    checkout_config_revision: 82,
                    ..Default::default()
                },
            )))
        });
        dom.process_events();
        assert_eq!(
            RUNS.with(std::cell::Cell::get),
            3,
            "fetched history remains tracked after target"
        );
        dom.in_runtime(|| {
            assert_eq!(
                f.targets
                    .choices
                    .offered_history
                    .peek()
                    .as_ref()
                    .unwrap()
                    .1
                    .checkout_config_revision,
                82
            )
        });
        PLACEMENT_FIXTURE.with(|slot| slot.set(None));
    }

    /// Relayed invocation deliberately differs from stored command metadata.
    /// Seeding must use prefill.invocation, while raw command/resume ownership
    /// and Clone repository membership retain their separate source contracts.
    fn source_prefill() -> CreatePrefill {
        let mut source =
            super::super::prefill_from(&super::super::super::row::row_specimen("source"), 1);
        source.host = Some(1);
        source.host_identity = Some(Some("install-1".into()));
        source.cwd = "/source/dir\u{1b}".into();
        source.title = "work".into();
        source.repo = Some(GithubRepo::parse("acme/repo").unwrap());
        source.invocation = "source-command\u{1b}".into();
        source.launch = None;
        source.command = Some(CommandLaunch {
            command: "metadata-command".into(),
            yolo: true,
            agent: Some(LaunchHarness::Claude),
            resume: Some("source-resume\u{1b}".into()),
        });
        source.mode = LauncherMode::Create;
        source
    }

    /// Source generation is an explicit boundary even under the real page claim.
    /// Later host binding/withdrawal must preserve literal edits and the original
    /// entry target; target-only retirement never publishes a detached snapshot.
    #[farhelm_testtrace::test]
    fn source_owner_separates_generation_host_and_target_lifetimes() {
        SYSTEM_FIXTURE.with(|slot| slot.set(None));
        PUBLICATIONS.with(|count| count.set(0));
        let mut dom = VirtualDom::new(system_fixture);
        dom.rebuild_in_place();
        dom.process_events();
        assert_eq!(
            PUBLICATIONS.with(std::cell::Cell::get),
            1,
            "aggregate oracle subscribed"
        );
        let SystemFixture {
            ordinary: OrdinaryFixture { owner, choices },
            mut source,
        } = SYSTEM_FIXTURE.with(std::cell::Cell::get).unwrap();
        let mut prefill = source_prefill();
        let mut ops = owner.ops;
        let mut key = owner.intent_key;
        dom.in_runtime(|| {
            owner.choose_launch_tab(LaunchTab::Command, choices);
            owner.edit_invocation("old-command".into());
            owner.choose_command_yolo(true);
            owner.edit_prompt("old prompt".into());
            owner.edit_conversation("keep-conversation".into());
            source
                .create_target
                .set(Some(CreateTarget::new(9, "entry-target".into())));
            source.browse_request.set(Some(BrowseAuthority {
                target: CreateTarget::new(9, "entry-target".into()),
                connection: Some(3),
                cwd: prefill.cwd.clone(),
                generation: 4,
            }));
            source.permission_escalation.set(Some(PermissionEscalation {
                generation: 1,
                opening_generation: 1,
                message: "old notice".into(),
                undo: owner
                    .capture_permission_switch(
                        &owner.snapshot(),
                        FINAL_CONTEXT
                            .with(std::cell::Cell::get)
                            .unwrap()
                            .search
                            .recovery,
                    )
                    .undo,
            }));
            key.set(Some(("before-source".into(), request(&owner.snapshot()))));
            assert_eq!(owner.snapshot().prompt().as_deref(), Some("old prompt"));
            assert_eq!(owner.snapshot().conversation(), "keep-conversation");
            assert!(source.browse_request.peek().is_some());
            assert!(source.error.peek().is_some());
            assert!(source.permission_escalation.peek().is_some());
            assert_eq!(*source.prefill_applied.peek(), None);
            assert!(key.peek().is_some());
        });
        dom.process_events();
        let before_seed = PUBLICATIONS.with(std::cell::Cell::get);
        let guard = dom.in_runtime(|| ops.claim_guard().expect("premise: claim the real token"));
        dom.in_runtime(|| {
            owner.reconcile_source(Some(&prefill), false, &[], source);
            assert!(ops.busy_now());
            assert!(key.peek().is_none());
            assert_eq!(owner.snapshot().prompt(), None);
            assert_eq!(owner.snapshot().conversation(), "keep-conversation");
            assert_eq!(owner.snapshot().submitted_cwd(), prefill.cwd);
            assert!(!owner.snapshot().cwd_edited());
            assert_ne!(owner.snapshot().cwd(), prefill.cwd);
            assert_eq!(command(&owner.snapshot()).command, prefill.invocation);
            assert_eq!(
                command(&owner.snapshot()).resume.as_deref(),
                Some("source-resume\u{1b}")
            );
            assert!(!owner.snapshot().invocation_edited());
            assert_eq!(owner.snapshot().destination().repo(), prefill.repo.as_ref());
            assert_eq!(*source.prefill_applied.peek(), Some(1));
            assert_eq!(*source.browse_generation.peek(), 5);
            assert!(source.browse_request.peek().is_none());
            assert!(source.browse_error.peek().is_none());
            assert!(source.error.peek().is_none() && source.preparation_error.peek().is_none());
            assert!(
                source.composer_reset_reason.peek().is_none()
                    && source.permission_escalation.peek().is_none()
            );
            assert_eq!(*source.preview_revision.peek(), 1);
            assert_eq!(*source.clone_host_state.peek(), CloneHostState::Waiting);
            assert_eq!(*owner.chosen_host.peek(), None);
            assert_eq!(
                *source.bound_target.peek(),
                Some(CreateTarget::new(9, "entry-target".into()))
            );
            assert!(*source.mount_seeded.peek());
        });
        dom.process_events();
        assert_eq!(PUBLICATIONS.with(std::cell::Cell::get), before_seed + 1);
        prefill.generation = 2;
        dom.in_runtime(|| {
            key.set(Some(("equal-source".into(), request(&owner.snapshot()))));
            owner.reconcile_source(Some(&prefill), false, &[], source);
            assert!(
                key.peek().is_none(),
                "equal bytes still belong to a new source generation"
            );
            assert_eq!(*source.preview_revision.peek(), 2);
        });
        dom.process_events();
        assert_eq!(PUBLICATIONS.with(std::cell::Cell::get), before_seed + 2);
        dom.in_runtime(|| drop(guard));
        dom.in_runtime(|| {
            owner.edit_invocation(owner.snapshot().invocation());
            assert!(owner.snapshot().invocation_edited());
            source.pending_initial_action.set(Some(
                crate::launch_composer::ComposerSearchResult::Name("new pending".into()),
            ));
            source.pending_initial_templates.write().push_back(
                farhelm_proto::launcher::LaunchTemplate {
                    name: "new pending".into(),
                    fields: Default::default(),
                },
            );
            source.partial_template_hint.set(true);
            source.auto_launch_pending.set(true);
            source.quick_launch_hidden.set(true);
        });
        dom.process_events();
        let before_host = PUBLICATIONS.with(std::cell::Cell::get);
        let host = super::super::super::shared::tests::option(1, "source host", false);
        let guard = dom.in_runtime(|| ops.claim_guard().unwrap());
        dom.in_runtime(|| {
            let literal = owner.snapshot();
            let pair = Some(("literal-edit".into(), request(&literal)));
            key.set(pair.clone());
            owner.reconcile_source(Some(&prefill), true, std::slice::from_ref(&host), source);
            assert_eq!(
                owner.snapshot(),
                literal,
                "late bind cannot reseed editor ownership"
            );
            assert_eq!(*owner.chosen_host.peek(), Some(1));
            assert_eq!(*source.clone_host_state.peek(), CloneHostState::Bound);
            assert_eq!(
                *source.bound_target.peek(),
                Some(CreateTarget::new(9, "entry-target".into()))
            );
            assert_eq!(
                *key.peek(),
                pair,
                "binding uses the already observed equal target"
            );
            assert!(source.pending_initial_action.peek().is_none());
            assert!(source.pending_initial_templates.peek().is_empty());
            assert!(
                !*source.partial_template_hint.peek()
                    && !*source.auto_launch_pending.peek()
                    && !*source.quick_launch_hidden.peek()
            );
            source
                .create_target
                .set(Some(CreateTarget::new(1, host.incarnation.clone())));
            source.error.set(Some("prior target refusal".into()));
            owner.reconcile_source(Some(&prefill), true, std::slice::from_ref(&host), source);
            assert_eq!(*source.bound_target.peek(), *source.create_target.peek());
            assert!(key.peek().is_none() && source.error.peek().is_none());
            assert_eq!(owner.snapshot(), literal);
            key.set(pair.clone());
            let mut moved = host.clone();
            moved.identity = Some("another-install".into());
            owner.reconcile_source(Some(&prefill), true, &[moved], source);
            assert_eq!(*owner.chosen_host.peek(), None);
            assert_eq!(
                *source.clone_host_state.peek(),
                CloneHostState::Unconfirmable
            );
            assert_eq!(owner.snapshot(), literal);
            assert_eq!(*key.peek(), pair);
            owner.reconcile_source(None, true, &[], source);
            assert_eq!(owner.snapshot(), literal);
            assert_eq!(
                *key.peek(),
                pair,
                "no-prefill equal target does no retirement"
            );
            source
                .create_target
                .set(Some(CreateTarget::new(2, "empty-source-target".into())));
            source.error.set(Some("old target refusal".into()));
            owner.reconcile_source(None, true, &[], source);
            assert_eq!(*source.bound_target.peek(), *source.create_target.peek());
            assert!(key.peek().is_none() && source.error.peek().is_none());
            assert_eq!(
                owner.snapshot(),
                literal,
                "empty source target change cannot reinstall fields"
            );
        });
        dom.process_events();
        assert_eq!(
            PUBLICATIONS.with(std::cell::Cell::get),
            before_host,
            "host/target-only work never publishes"
        );
        dom.in_runtime(|| {
            let mut agent = LaunchDraft::new(None);
            agent.choose_harness(
                LaunchHarness::Codex,
                None,
                String::new(),
                ChoiceOrigin::Control,
            );
            prefill.generation = 3;
            prefill.launch = agent.structured_selection();
            prefill.command = None;
            prefill.mode = LauncherMode::Replace {
                source: "source".into(),
                opened: (crate::SessionStatus::Unknown, 0),
            };
            key.set(Some((
                "before-agent-source".into(),
                request(&owner.snapshot()),
            )));
            owner.reconcile_source(Some(&prefill), false, &[], source);
            assert_eq!(structured(&owner.snapshot()).harness, LaunchHarness::Codex);
            assert!(
                !owner.snapshot().destination().is_managed(),
                "Replace with ignores source repository membership"
            );
            assert_eq!(owner.snapshot().submitted_cwd(), prefill.cwd);
            assert_eq!(owner.snapshot().title_raw_seed().as_deref(), Some("work"));
            assert_eq!(
                owner.snapshot().invocation_raw_seed().as_deref(),
                Some(prefill.invocation.as_str())
            );
            assert_eq!(owner.snapshot().command_yolo(), None);
            assert_eq!(owner.snapshot().command_agent(), None);
            assert_eq!(owner.snapshot().prompt(), None);
            assert_eq!(owner.snapshot().conversation(), "keep-conversation");
            assert!(key.peek().is_none());
        });
        dom.process_events();
        assert_eq!(PUBLICATIONS.with(std::cell::Cell::get), before_host + 1);
        dom.in_runtime(|| drop(guard));
        SYSTEM_FIXTURE.with(|slot| slot.set(None));
    }

    /// Offer the live resolved title without reproducing its naming algorithm.
    /// Other authority fields are fixed external facts, as in the response effect.
    fn naming_authority(draft: &LaunchDraft) -> PreviewAuthority {
        PreviewAuthority {
            generation: 2,
            config_revision: 7,
            host: "1".into(),
            incarnation: 4,
            installation_identity: "install-a".into(),
            repo: draft.destination().repo().unwrap().clone(),
            title: Some(draft.effective_title()),
            agent: "fixture-agent".into(),
        }
    }

    /// System naming must preserve authority while busy and publish only change.
    /// The private Signal oracle distinguishes a refused/equal reconciliation
    /// from accidentally publishing an equal value behind a filtered field Memo.
    #[farhelm_testtrace::test]
    fn naming_owner_freezes_retained_attempts_and_skips_refused_publications() {
        SYSTEM_FIXTURE.with(|slot| slot.set(None));
        PUBLICATIONS.with(|count| count.set(0));
        let mut dom = VirtualDom::new(system_fixture);
        dom.rebuild_in_place();
        dom.process_events();
        let SystemFixture {
            ordinary: OrdinaryFixture { owner, .. },
            source,
        } = SYSTEM_FIXTURE.with(std::cell::Cell::get).unwrap();
        let prefill = source_prefill();
        let target = Some(CreateTarget::new(1, "install-a".into()));
        dom.in_runtime(|| {
            owner.reconcile_source(Some(&prefill), false, &[], source);
            owner.retarget_checkout_name(target.clone());
        });
        dom.process_events();
        let baseline = PUBLICATIONS.with(std::cell::Cell::get);
        let authority = dom.in_runtime(|| naming_authority(&owner.snapshot()));
        let occupied = api::GithubPreviewFailure {
            message: "occupied".into(),
            occupied: true,
        };
        let mut ops = owner.ops;
        let guard = dom.in_runtime(|| {
            ops.claim_guard()
                .expect("premise: naming runs under the real claim")
        });
        let mut key = owner.intent_key;
        dom.in_runtime(|| {
            assert_eq!(
                owner.snapshot().checkout_naming().default_title(
                    owner.snapshot().title_edited(),
                    owner.snapshot().title_raw_seed().as_deref(),
                    owner.snapshot().destination().repo(),
                ),
                authority.title,
                "premise: the offer names the current default"
            );
            let preview = accepted_preview();
            let checkout = GithubCheckoutRequest {
                repo: authority.repo.identifier(),
                title: authority.title.clone(),
                preview: preview.clone(),
            };
            let current = owner.snapshot();
            let retained = current
                .request(RequestContext {
                    host: Some((1, "fixture-incarnation")),
                    launch: current.launch_intent().unwrap(),
                    replace_source: None,
                    checkout: Some(CheckoutInput::Accepted {
                        cwd: preview.cwd.raw(),
                        request: &checkout,
                    }),
                })
                .unwrap();
            let reconciling = super::super::same_fresh_intent(
                &retained,
                &request(&current),
                &authority.repo,
                "install-a",
            );
            assert!(
                reconciling,
                "premise: production comparison matches the retained accepted checkout"
            );
            let pair = Some(("retained-checkout".into(), retained));
            key.set(pair.clone());
            assert!(!owner.advance_checkout_name_for_occupied_reply(
                &authority,
                &authority,
                &occupied,
                reconciling
            ));
            let mut stale = authority.clone();
            stale.generation += 1;
            assert!(
                !owner
                    .advance_checkout_name_for_occupied_reply(&stale, &authority, &occupied, false)
            );
            let mut wrong_title = authority.clone();
            wrong_title.title = Some("another title".into());
            assert!(!owner.advance_checkout_name_for_occupied_reply(
                &wrong_title,
                &wrong_title,
                &occupied,
                false
            ));
            let not_occupied = api::GithubPreviewFailure {
                message: "transport failure".into(),
                occupied: false,
            };
            assert!(!owner.advance_checkout_name_for_occupied_reply(
                &authority,
                &authority,
                &not_occupied,
                false
            ));
            assert_eq!(*key.peek(), pair);
            assert!(ops.busy_now());
        });
        dom.process_events();
        assert_eq!(PUBLICATIONS.with(std::cell::Cell::get), baseline);
        dom.in_runtime(|| {
            // A fresh offer has no retained attempt to freeze. Its ordinary
            // slot still survives name resolution until submit compares it.
            let pair = Some(("fresh-offer".into(), request(&owner.snapshot())));
            key.set(pair.clone());
            assert!(owner.advance_checkout_name_for_occupied_reply(
                &authority, &authority, &occupied, false
            ));
            assert_ne!(
                owner.snapshot().effective_title(),
                authority.title.clone().unwrap()
            );
            assert_eq!(
                request(&owner.snapshot()).title,
                owner.snapshot().effective_title()
            );
            assert_eq!(*key.peek(), pair);
        });
        dom.process_events();
        assert_eq!(
            PUBLICATIONS.with(std::cell::Cell::get),
            baseline + 1,
            "real advance publishes"
        );
        dom.in_runtime(|| owner.retarget_checkout_name(target));
        dom.process_events();
        assert_eq!(
            PUBLICATIONS.with(std::cell::Cell::get),
            baseline + 1,
            "equal retarget publishes nothing"
        );
        dom.in_runtime(|| {
            let pair = key.peek().clone();
            owner.retarget_checkout_name(Some(CreateTarget::new(1, "install-b".into())));
            assert_eq!(Some(owner.snapshot().effective_title()), authority.title);
            assert_eq!(*key.peek(), pair);
        });
        dom.process_events();
        assert_eq!(
            PUBLICATIONS.with(std::cell::Cell::get),
            baseline + 2,
            "changed target really publishes its reset"
        );
        dom.in_runtime(|| drop(guard));
        dom.in_runtime(|| {
            apply_fields(
                owner,
                farhelm_proto::launcher::TemplateFields {
                    name: Some("fixed".into()),
                    name_kind: Some(farhelm_proto::launcher::TemplateNameKind::Fixed),
                    ..Default::default()
                },
            )
        });
        dom.process_events();
        let fixed_baseline = PUBLICATIONS.with(std::cell::Cell::get);
        let guard = dom.in_runtime(|| ops.claim_guard().unwrap());
        dom.in_runtime(|| {
            let fixed = naming_authority(&owner.snapshot());
            let pair = key.peek().clone();
            assert!(
                !owner.advance_checkout_name_for_occupied_reply(&fixed, &fixed, &occupied, false)
            );
            assert_eq!(*key.peek(), pair);
        });
        dom.process_events();
        assert_eq!(PUBLICATIONS.with(std::cell::Cell::get), fixed_baseline);
        dom.in_runtime(|| drop(guard));
        dom.in_runtime(|| {
            assert!(owner.edit_title("manual".into()));
            key.set(Some(("manual".into(), request(&owner.snapshot()))));
        });
        dom.process_events();
        let manual_baseline = PUBLICATIONS.with(std::cell::Cell::get);
        let guard = dom.in_runtime(|| ops.claim_guard().unwrap());
        dom.in_runtime(|| {
            let manual = naming_authority(&owner.snapshot());
            let pair = key.peek().clone();
            assert!(
                !owner.advance_checkout_name_for_occupied_reply(&manual, &manual, &occupied, false)
            );
            assert_eq!(*key.peek(), pair);
        });
        dom.process_events();
        assert_eq!(PUBLICATIONS.with(std::cell::Cell::get), manual_baseline);
        dom.in_runtime(|| drop(guard));
        dom.in_runtime(|| {
            // Reach the cap through the existing pure engine, not a fixture
            // copy of its index/math. Detailed series expectations live below.
            apply_fields(
                owner,
                farhelm_proto::launcher::TemplateFields {
                    name: Some("base".into()),
                    name_kind: Some(farhelm_proto::launcher::TemplateNameKind::Base),
                    ..Default::default()
                },
            );
            for _ in 0..49 {
                assert!(advance_fixture_name(owner));
            }
            assert!(
                !advance_fixture_name(owner),
                "premise: real sequence is capped"
            );
            key.set(Some(("capped".into(), request(&owner.snapshot()))));
        });
        dom.process_events();
        let cap_baseline = PUBLICATIONS.with(std::cell::Cell::get);
        let guard = dom.in_runtime(|| ops.claim_guard().unwrap());
        dom.in_runtime(|| {
            let capped = naming_authority(&owner.snapshot());
            let pair = key.peek().clone();
            assert!(
                !owner.advance_checkout_name_for_occupied_reply(&capped, &capped, &occupied, false)
            );
            assert_eq!(*key.peek(), pair);
        });
        dom.process_events();
        assert_eq!(PUBLICATIONS.with(std::cell::Cell::get), cap_baseline);
        dom.in_runtime(|| drop(guard));
        SYSTEM_FIXTURE.with(|slot| slot.set(None));
    }

    /// Source observes the target, never aggregate edits or its own host aliases.
    /// Positive target reruns and seed publication establish that the absence of
    /// draft/host reruns is an ownership contract rather than an inert fixture.
    #[farhelm_testtrace::test]
    fn source_owner_retains_target_subscription_without_draft_or_host_tracking() {
        std::thread_local! {
            static RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        }
        /// Keep one immutable source generation, as the form's effect captures
        /// its prop. Host/latch reads inside that source branch must remain peeks;
        /// the target is the only reactive dependency earned by this body.
        fn probe() -> Element {
            let SystemFixture {
                ordinary: OrdinaryFixture { owner, .. },
                source,
            } = use_system_fixture();
            let prefill = use_hook(source_prefill);
            use_effect(move || {
                RUNS.with(|count| count.set(count.get() + 1));
                owner.reconcile_source(Some(&prefill), false, &[], source);
            });
            rsx! {}
        }
        RUNS.with(|count| count.set(0));
        PUBLICATIONS.with(|count| count.set(0));
        SYSTEM_FIXTURE.with(|slot| slot.set(None));
        let mut dom = VirtualDom::new(probe);
        dom.rebuild_in_place();
        dom.process_events();
        let SystemFixture {
            ordinary: OrdinaryFixture { owner, .. },
            mut source,
        } = SYSTEM_FIXTURE.with(std::cell::Cell::get).unwrap();
        assert_eq!(RUNS.with(std::cell::Cell::get), 1);
        let initial_publications = PUBLICATIONS.with(std::cell::Cell::get);
        assert!(
            initial_publications > 0,
            "aggregate oracle subscribed and settled with source seed"
        );
        dom.in_runtime(|| {
            assert!(*source.mount_seeded.peek());
            assert_eq!(*source.prefill_applied.peek(), Some(1));
            assert_eq!(
                owner.snapshot().invocation_raw_seed().as_deref(),
                Some(source_prefill().invocation.as_str())
            );
            assert_eq!(*source.bound_target.peek(), *source.create_target.peek());
            owner.edit_prompt("unrelated draft".into());
            let mut chosen = owner.chosen_host;
            chosen.set(Some(3));
        });
        dom.process_events();
        assert_eq!(RUNS.with(std::cell::Cell::get), 1);
        assert_eq!(
            PUBLICATIONS.with(std::cell::Cell::get),
            initial_publications + 1
        );
        dom.in_runtime(|| {
            source
                .create_target
                .set(Some(CreateTarget::new(2, "install-b".into())))
        });
        dom.process_events();
        assert_eq!(RUNS.with(std::cell::Cell::get), 2, "target remains tracked");
        assert_eq!(
            PUBLICATIONS.with(std::cell::Cell::get),
            initial_publications + 1,
            "target-only work does not publish"
        );
        dom.in_runtime(|| {
            let mut next = source_prefill();
            next.generation = 2;
            owner.reconcile_source(Some(&next), false, &[], source);
        });
        dom.process_events();
        assert_eq!(
            PUBLICATIONS.with(std::cell::Cell::get),
            initial_publications + 2,
            "seed is a positive publication control"
        );
        assert_eq!(
            RUNS.with(std::cell::Cell::get),
            2,
            "seed/host writes do not subscribe or retrigger source"
        );
        SYSTEM_FIXTURE.with(|slot| slot.set(None));
    }

    /// A queued callback must reject before it claims text or promotes history.
    /// Every concrete ordinary entry uses the real held token in this turn;
    /// equal admitted choices still complete, while a real change retires K.
    #[farhelm_testtrace::test]
    fn ordinary_owner_gates_all_entries_and_keeps_equal_choice_completion() {
        ORDINARY_FIXTURE.with(|slot| slot.set(None));
        let mut dom = VirtualDom::new(ordinary_fixture);
        dom.rebuild_in_place();
        dom.process_events();
        let OrdinaryFixture { owner, mut choices } =
            ORDINARY_FIXTURE.with(std::cell::Cell::get).unwrap();
        dom.in_runtime(|| {
            seed_invocation_from_source(owner, "source\u{1b}");
            apply_fields(
                owner,
                farhelm_proto::launcher::TemplateFields {
                    kind: Some(farhelm_proto::launcher::LauncherKind::Agent),
                    agent: Some(LaunchHarness::Codex),
                    permissions: Some(Some(LaunchPermission::Custom)),
                    permission_flags: Some(vec!["--mode".into(), "ask me".into()]),
                    wrapper_prefix: Some(Some(vec!["nice".into(), "two words".into()])),
                    ..Default::default()
                },
            );
            choices.partial_template_hint.set(true);
            choices.offered_history.set(Some((
                CreateTarget::new(1, "install-a".into()),
                api::LaunchHistory {
                    checkout_config_revision: 1,
                    ..Default::default()
                },
            )));
            let mut key = owner.intent_key;
            let pair = Some(("unresolved".into(), request(&owner.snapshot())));
            key.set(pair.clone());
            let before = owner.snapshot();
            let offered = choices.offered_history.peek().clone();
            let mut ops = owner.ops;
            let guard = ops
                .claim_guard()
                .expect("premise: the real page token is idle");
            assert!(ops.busy_now());
            assert!(!owner.edit_title("changed".into()));
            assert!(!owner.edit_invocation("changed".into()));
            assert!(!owner.edit_command_resume("changed".into()));
            assert!(!owner.edit_prompt("changed".into()));
            assert!(!owner.edit_conversation("changed".into()));
            assert!(!owner.edit_wrapper("changed".into()));
            assert!(!owner.edit_permission_flags("changed".into()));
            assert!(!owner.choose_command_yolo(true));
            assert!(!owner.choose_command_agent(Some(LaunchHarness::Claude)));
            assert!(!owner.choose_command_resume(true));
            assert!(!owner.choose_effort(Some(LaunchEffort::High), choices));
            assert!(!owner.choose_permissions(Some(LaunchPermission::Guarded), choices));
            assert!(!owner.choose_launch_tab(LaunchTab::Command, choices));
            assert!(!owner.navigate_launch_tab(LaunchTab::Command, choices));
            assert_eq!(
                owner.snapshot(),
                before,
                "rejection preserves all raw/edited fields"
            );
            assert_eq!(*key.peek(), pair);
            assert!(*choices.partial_template_hint.peek());
            assert_eq!(*choices.offered_history.peek(), offered);

            drop(guard);
            assert!(!ops.busy_now(), "the production guard releases admission");
            assert!(owner.choose_launch_tab(LaunchTab::Agent, choices));
            assert!(!*choices.partial_template_hint.peek());
            assert_eq!(
                *choices.offered_history.peek(),
                *choices.fetched_history.peek()
            );
            assert_eq!(*key.peek(), pair, "equal tab still completed its preamble");
            choices.partial_template_hint.set(true);
            choices.offered_history.set(offered);
            assert!(owner.navigate_launch_tab(LaunchTab::Agent, choices));
            assert!(!*choices.partial_template_hint.peek());
            assert_eq!(
                *choices.offered_history.peek(),
                *choices.fetched_history.peek()
            );
            assert!(owner.choose_effort(None, choices));
            assert!(owner.choose_permissions(Some(LaunchPermission::Custom), choices));
            assert!(owner.edit_permission_flags("--mode \"ask me\"".into()));
            assert!(owner.edit_wrapper("nice \"two words\"".into()));
            assert!(owner.snapshot().permissions_explicit());
            assert_eq!(owner.snapshot().permission_flags(), "--mode \"ask me\"");
            assert_eq!(
                *key.peek(),
                pair,
                "word-equivalent edits retain the full pair"
            );
            assert!(owner.choose_effort(Some(LaunchEffort::High), choices));
            assert!(
                key.peek().is_none(),
                "a changed submitted choice retires before return"
            );
            assert_eq!(
                *owner.fields().structured_effort.peek(),
                None,
                "premise: Memo work has not run"
            );
            assert_eq!(
                structured(&owner.snapshot()).effort,
                Some(LaunchEffort::High)
            );
        });
        ORDINARY_FIXTURE.with(|slot| slot.set(None));
    }

    /// Source bytes and visible escape spelling are different commands.
    /// Live owner publication must beat Memo scheduling, yet hidden Resume
    /// ownership and repeated literal edits still preserve the unresolved pair.
    #[farhelm_testtrace::test]
    fn ordinary_owner_keeps_raw_command_and_hidden_resume_contracts() {
        ORDINARY_FIXTURE.with(|slot| slot.set(None));
        let mut dom = VirtualDom::new(ordinary_fixture);
        dom.rebuild_in_place();
        dom.process_events();
        let OrdinaryFixture { owner, choices } =
            ORDINARY_FIXTURE.with(std::cell::Cell::get).unwrap();
        dom.in_runtime(|| {
            seed_invocation_from_source(owner, "c\u{1b}");
            assert!(owner.choose_launch_tab(LaunchTab::Command, choices));
            assert!(owner.choose_command_yolo(true));
            let mut key = owner.intent_key;
            key.set(Some(("raw-command".into(), request(&owner.snapshot()))));
            let display = owner.snapshot().invocation();
            assert_ne!(command(&owner.snapshot()).command, display);
            assert!(owner.edit_invocation(display.clone()));
            assert!(key.peek().is_none());
            assert_eq!(
                owner.fields().invocation.peek().as_str(),
                "",
                "premise: the display Memo is still old"
            );
            assert_eq!(command(&owner.snapshot()).command, display);
            assert!(owner.snapshot().invocation_edited());
            let pair = Some(("literal-command".into(), request(&owner.snapshot())));
            key.set(pair.clone());
            assert!(owner.edit_invocation(display));
            assert!(owner.choose_command_yolo(true));
            assert!(owner.choose_command_agent(None));
            assert!(owner.edit_command_resume("literal resume".into()));
            assert!(owner.choose_command_resume(true));
            assert!(owner.snapshot().command_resume_edited());
            assert!(owner.snapshot().command_resume_on());
            assert_eq!(command(&owner.snapshot()).resume, None);
            assert_eq!(
                *key.peek(),
                pair,
                "hidden Resume assignments still complete"
            );
            assert!(owner.choose_command_agent(Some(LaunchHarness::Claude)));
            assert!(key.peek().is_none());
            assert_eq!(
                command(&owner.snapshot()).resume.as_deref(),
                Some("literal resume")
            );
        });
        ORDINARY_FIXTURE.with(|slot| slot.set(None));
    }

    /// History reads stay conditional on admission and remain tracked afterward.
    /// An all-peek rewrite loses the positive reruns; reading context before the
    /// gate would make the first, rejected effect subscribe to old history.
    #[farhelm_testtrace::test]
    fn ordinary_choice_history_dependencies_begin_only_after_admission() {
        use std::cell::Cell;
        /// The held guard and explicit retry stimulus belong to this one DOM.
        /// Keeping their handles together makes the rejected/admitted effect
        /// boundary observable without changing the production owner API.
        #[derive(Clone, Copy)]
        struct HistoryFixture {
            ordinary: OrdinaryFixture,
            claim: Signal<Option<crate::ops::OpGuard>>,
            kick: Signal<u64>,
        }
        std::thread_local! {
            static FIXTURE: Cell<Option<HistoryFixture>> = const { Cell::new(None) };
            static RUNS: Cell<usize> = const { Cell::new(0) };
            static ACCEPTED: Cell<usize> = const { Cell::new(0) };
        }
        /// Hold a production claim before the effect's first invocation. Kick
        /// is the explicit retry boundary after rejection acquired no history deps.
        fn probe() -> Element {
            let mut ops = crate::ops::use_op_lock();
            let key = use_signal(|| None);
            let chosen_host = use_signal(|| None);
            let owner = use_launch_draft(None, None, ops, key, chosen_host);
            let choices = use_choice_targets();
            let source = use_source_targets(choices);
            use_placement_handles(SystemFixture {
                ordinary: OrdinaryFixture { owner, choices },
                source,
            });
            let mut claim = use_signal(|| None);
            let kick = use_signal(|| 0_u64);
            use_hook(move || {
                claim.set(Some(
                    ops.claim_guard()
                        .expect("premise: mount claims the real token"),
                ));
            });
            use_effect(move || {
                let _ = kick();
                RUNS.with(|count| count.set(count.get() + 1));
                if owner.choose_effort(None, choices) {
                    ACCEPTED.with(|count| count.set(count.get() + 1));
                }
            });
            FIXTURE.with(|slot| {
                slot.set(Some(HistoryFixture {
                    ordinary: OrdinaryFixture { owner, choices },
                    claim,
                    kick,
                }))
            });
            rsx! {}
        }
        FIXTURE.with(|slot| slot.set(None));
        RUNS.with(|count| count.set(0));
        ACCEPTED.with(|count| count.set(0));
        let mut dom = VirtualDom::new(probe);
        dom.rebuild_in_place();
        dom.process_events();
        let HistoryFixture {
            ordinary: OrdinaryFixture { owner, mut choices },
            mut claim,
            mut kick,
        } = FIXTURE.with(Cell::get).unwrap();
        assert_eq!(RUNS.with(Cell::get), 1);
        assert_eq!(ACCEPTED.with(Cell::get), 0);
        dom.in_runtime(|| {
            assert!(
                owner.ops.busy_now(),
                "premise: the first effect really was rejected"
            );
            assert!(*choices.partial_template_hint.peek());
            choices.fetched_history.set(None);
            choices.create_target.set(None);
        });
        dom.process_events();
        assert_eq!(
            RUNS.with(Cell::get),
            1,
            "busy admission acquired no history subscriptions"
        );
        dom.in_runtime(|| {
            claim.set(None);
            assert!(!owner.ops.busy_now());
            kick.set(1);
        });
        dom.process_events();
        assert_eq!(RUNS.with(Cell::get), 2);
        assert_eq!(ACCEPTED.with(Cell::get), 1);
        dom.in_runtime(|| {
            choices
                .create_target
                .set(Some(CreateTarget::new(2, "install-b".into())))
        });
        dom.process_events();
        assert_eq!(
            RUNS.with(Cell::get),
            3,
            "the admitted helper tracks current target"
        );
        dom.in_runtime(|| {
            choices.fetched_history.set(Some((
                CreateTarget::new(2, "install-b".into()),
                api::LaunchHistory {
                    checkout_config_revision: 4,
                    ..Default::default()
                },
            )))
        });
        dom.process_events();
        assert_eq!(
            RUNS.with(Cell::get),
            4,
            "the admitted helper tracks fetched history"
        );
        dom.in_runtime(|| {
            assert_eq!(
                *choices.offered_history.peek(),
                *choices.fetched_history.peek()
            );
            assert!(owner.edit_prompt("unrelated owner field".into()));
        });
        dom.process_events();
        assert_eq!(
            RUNS.with(Cell::get),
            4,
            "history choice has no whole-owner subscription"
        );
        FIXTURE.with(|slot| slot.set(None));
    }

    /// Choices can claim editor/template ownership without changing the create.
    /// Exercise production facts alongside the resulting submitted selection;
    /// a no-op fact must not skip the underlying accepted assignments.
    #[test]
    fn repeated_choices_preserve_requests_and_apply_ownership() {
        let mut draft = LaunchDraft::new(None);
        assert!(
            draft
                .choose_harness(
                    LaunchHarness::Codex,
                    None,
                    String::new(),
                    ChoiceOrigin::Control
                )
                .selection_changed
        );
        let before = structured(&draft);
        assert!(!draft.choose_tab(LaunchTab::Agent));
        for origin in [ChoiceOrigin::Control, ChoiceOrigin::Search] {
            assert!(
                !draft
                    .choose_harness(LaunchHarness::Codex, None, "ignored".into(), origin)
                    .selection_changed
            );
            assert_eq!(structured(&draft), before);
        }
        assert!(!draft.choose_default_model());
        assert!(
            draft
                .choose_custom_model("private".into(), LaunchHarness::Codex, None)
                .selection_changed
        );
        assert!(
            !draft
                .choose_custom_model("private".into(), LaunchHarness::Codex, None)
                .selection_changed
        );
        assert_eq!(draft.model_owner(), Some(LaunchHarness::Codex));
        for origin in [ChoiceOrigin::Control, ChoiceOrigin::Search] {
            assert!(
                !draft
                    .choose_catalog_model(
                        "private".into(),
                        LaunchHarness::Codex,
                        None,
                        String::new(),
                        origin
                    )
                    .selection_changed
            );
        }
        assert!(draft.choose_effort(Some(LaunchEffort::High)));
        assert!(!draft.choose_effort(Some(LaunchEffort::High)));
        let outcome = draft.choose_custom_model("private".into(), LaunchHarness::Codex, Some(&[]));
        assert!(outcome.selection_changed && outcome.reset_reason.is_some());
        assert_eq!(
            draft.effort(),
            None,
            "known catalog reconciliation is a real change"
        );
        assert!(draft.choose_default_model());
        assert!(!draft.choose_default_model());
        assert!(
            draft
                .choose_harness(
                    LaunchHarness::Claude,
                    None,
                    String::new(),
                    ChoiceOrigin::Control
                )
                .selection_changed
        );
    }

    /// Accepted raw model bytes, rather than the escaped display, identify the
    /// create. Claiming an identical-looking label can therefore be a real edit.
    #[test]
    fn custom_model_fact_compares_accepted_raw_bytes() {
        let mut draft = LaunchDraft::new(None);
        let raw = "private\u{202e}";
        let selection = LaunchSelection {
            harness: LaunchHarness::Codex,
            model: Some(raw.into()),
            effort: None,
            permissions: Some(LaunchPermission::Guarded),
            permission_flags: None,
            wrapper_prefix: None,
            workspace_trust: None,
        };
        draft.restore_launch(&selection, Some(LaunchHarness::Codex));
        let visible = display_peer(raw);
        assert_ne!(visible, raw, "premise: relayed model requires escaping");
        assert_eq!(structured(&draft).model.as_deref(), Some(raw));
        assert!(
            draft
                .choose_custom_model(visible.clone(), LaunchHarness::Codex, None)
                .selection_changed
        );
        assert_eq!(structured(&draft).model.as_deref(), Some(visible.as_str()));
        assert_eq!(draft.model_raw_seed(), None);
        assert!(draft.model_edited());
        assert!(
            !draft
                .choose_custom_model(visible, LaunchHarness::Codex, None)
                .selection_changed
        );
    }

    /// Request defaults and successfully parsed words establish equality; raw
    /// permission omission, quoting and explicitness are retained editor state.
    /// Invalid parsers cannot establish that a different editor is a safe retry.
    #[test]
    fn permission_and_word_facts_follow_valid_submitted_values() {
        let mut draft = LaunchDraft::new(None);
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        draft.apply_template_permissions(None, "dormant".into());
        let before = structured(&draft);
        assert_eq!(draft.permissions(), None);
        assert_eq!(before.permissions, Some(LaunchPermission::Guarded));
        assert!(!draft.choose_permissions(Some(LaunchPermission::Guarded)));
        assert_eq!(structured(&draft), before);
        assert!(draft.permissions_explicit());
        assert_eq!(draft.permission_flags(), "");
        assert!(draft.choose_permissions(Some(LaunchPermission::Yolo)));
        assert!(!draft.choose_permissions(Some(LaunchPermission::Yolo)));
        assert!(draft.choose_permissions(Some(LaunchPermission::Custom)));
        assert!(draft.edit_permission_flags("--mode 'ask me'".into()));
        let custom = structured(&draft);
        assert!(!draft.edit_permission_flags("--mode \"ask me\" ".into()));
        assert_eq!(structured(&draft), custom);
        assert_eq!(draft.permission_flags(), "--mode \"ask me\" ");
        assert!(!draft.choose_permissions(Some(LaunchPermission::Custom)));
        assert!(draft.edit_permission_flags("--mode deny".into()));
        assert!(draft.edit_permission_flags("'unfinished".into()));
        assert!(draft.edit_permission_flags("'unfinished".into()));
        assert!(draft.choose_permissions(Some(LaunchPermission::Custom)));
        assert!(draft.edit_permission_flags("# no words".into()));
        assert!(draft.edit_wrapper("wrapper 'two words'".into()));
        let wrapped = structured(&draft);
        assert!(!draft.edit_wrapper("wrapper \"two words\" ".into()));
        assert_eq!(structured(&draft), wrapped);
        assert!(draft.wrapper_explicit());
        assert_eq!(draft.wrapper(), "wrapper \"two words\" ");
        assert!(draft.edit_wrapper("wrapper other".into()));
        assert!(draft.edit_wrapper(" ".into()));
        assert!(draft.edit_wrapper(" ".into()));
        assert!(draft.edit_wrapper(String::new()));
        assert!(!draft.edit_wrapper(String::new()));
    }

    /// Ordinary relayed placement can change its ownership without changing its
    /// operational path. Returning from managed mode is a semantic change even
    /// when the dormant folder happens to contain the chosen raw bytes.
    #[test]
    fn placement_facts_keep_operational_paths_and_semantic_mode() {
        let mut draft = LaunchDraft::new(None);
        draft.seed_folder("/work");
        assert!(!draft.edit_folder("/work".into()));
        assert!(draft.cwd_edited() && draft.folder_is_explicit());
        assert!(!draft.choose_existing_folder("/work"));
        assert!(!draft.cwd_edited() && draft.folder_is_explicit());
        draft.seed_folder("/work\u{202e}");
        let visible = draft.cwd();
        assert_ne!(visible, draft.submitted_cwd());
        assert!(draft.edit_folder(visible.clone()));
        assert_eq!(draft.submitted_cwd(), visible);
        assert!(draft.choose_existing_folder("/work"));
        draft.edit_repository_query("acme/repo".into());
        assert!(draft.choose_existing_folder("/work"));
        draft.select_repository(GithubRepo::parse("acme/repo").unwrap());
        assert!(draft.edit_folder("/work".into()));
        assert!(!draft.edit_folder("/work".into()));
    }

    /// Blank ordinary names are the same omitted API title, but a nonblank
    /// title's spacing and every managed preview/accepted title remain literal.
    /// Use the sole production envelope builder, including its candidate case.
    #[test]
    fn ordinary_optional_title_identity_does_not_normalize_managed_titles() {
        let mut draft = LaunchDraft::new(None);
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        let blank = request(&draft);
        assert!(!draft.edit_title(" ".into()));
        assert_eq!(draft.title(), " ");
        assert!(draft.title_edited());
        assert_eq!(request(&draft), blank);
        assert!(draft.edit_title(" work ".into()));
        assert_eq!(request(&draft).title, " work ");
        assert!(!draft.edit_title(" work ".into()));
        assert!(draft.edit_title("work".into()));
        draft.edit_repository_query("acme/repo".into());
        assert!(draft.edit_title(" ".into()));
        assert_eq!(
            request(&draft).title,
            " ",
            "a query without accepted checkout is still managed"
        );
        let repo = GithubRepo::parse("acme/repo").unwrap();
        draft.select_repository(repo.clone());
        assert!(
            draft.edit_title(" ".into()),
            "managed naming keeps conservative retirement"
        );
        let preview = accepted_preview();
        let managed = draft
            .request(RequestContext {
                host: Some((1, "fixture-incarnation")),
                launch: draft.launch_intent().unwrap(),
                replace_source: None,
                checkout: Some(CheckoutInput::Preview {
                    repo: &repo,
                    preview: &preview,
                }),
            })
            .unwrap();
        let checkout = managed.github_checkout.as_ref().unwrap();
        assert_eq!(checkout.title.as_deref(), Some(" "));
        draft.edit_title("new name".into());
        let accepted = draft
            .request(RequestContext {
                host: Some((1, "fixture-incarnation")),
                launch: draft.launch_intent().unwrap(),
                replace_source: None,
                checkout: Some(CheckoutInput::Accepted {
                    cwd: preview.cwd.raw(),
                    request: checkout,
                }),
            })
            .unwrap();
        assert_eq!(accepted.github_checkout, managed.github_checkout);
        assert_eq!(accepted.title, "new name");
    }

    /// Command strings are literal, while hidden Resume is deliberately absent
    /// from the request. Claiming an escaped seed must still change actual bytes.
    #[test]
    fn command_facts_keep_literal_bytes_and_effective_resume() {
        let mut draft = LaunchDraft::new(None);
        draft.choose_tab(LaunchTab::Command);
        assert!(draft.choose_command_yolo(false));
        assert!(!draft.choose_command_yolo(false));
        assert!(draft.edit_invocation("run".into()));
        assert!(!draft.edit_invocation("run".into()));
        assert!(draft.edit_invocation("run ".into()));
        draft.seed_invocation("run\u{202e}");
        let visible = draft.invocation();
        assert_ne!(command(&draft).command, visible);
        assert!(draft.edit_invocation(visible.clone()));
        assert_eq!(command(&draft).command, visible);
        assert!(draft.invocation_edited());
        assert!(!draft.choose_command_agent(None));
        assert!(!draft.edit_command_resume("resume".into()));
        assert!(!draft.choose_command_resume(true));
        assert_eq!(command(&draft).resume, None);
        assert!(draft.choose_command_agent(Some(LaunchHarness::Codex)));
        assert!(!draft.choose_command_agent(Some(LaunchHarness::Codex)));
        assert_eq!(command(&draft).resume.as_deref(), Some("resume"));
        assert!(!draft.edit_command_resume("resume".into()));
        assert!(draft.edit_command_resume("resume ".into()));
        assert!(draft.choose_command_resume(false));
        assert!(!draft.choose_command_resume(false));
        assert!(!draft.edit_command_resume("retained".into()));
        assert_eq!(draft.command_resume(), "retained");
        assert_eq!(command(&draft).resume, None);
        draft.seed_command_fields(Some(&CommandLaunch {
            command: "run".into(),
            yolo: false,
            agent: Some(LaunchHarness::Codex),
            resume: Some("resume\u{202e}".into()),
        }));
        let visible = draft.command_resume();
        assert_ne!(command(&draft).resume.as_deref(), Some(visible.as_str()));
        assert!(draft.edit_command_resume(visible.clone()));
        assert_eq!(command(&draft).resume.as_deref(), Some(visible.as_str()));
    }

    /// One-time text keeps literal values and template presence. A recent's
    /// ownership refresh can likewise leave its normalized structured request
    /// unchanged, but invalid old words cannot prove that equality.
    #[test]
    fn one_time_inputs_and_recent_facts_use_production_projections() {
        let mut draft = LaunchDraft::new(None);
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        assert!(draft.edit_prompt("hello".into()));
        assert!(!draft.edit_prompt("hello".into()));
        assert!(draft.edit_prompt("hello ".into()));
        draft.apply_template_prompt(Some(String::new()));
        assert_eq!(request(&draft).prompt, Some(String::new()));
        assert!(draft.edit_prompt(String::new()));
        assert_eq!(request(&draft).prompt, None);
        assert!(!draft.edit_prompt(String::new()));
        assert!(draft.edit_conversation("conversation-1".into()));
        assert!(!draft.edit_conversation("conversation-1".into()));
        assert!(draft.edit_conversation("conversation-1 ".into()));
        assert_eq!(
            request(&draft).conversation.as_deref(),
            Some("conversation-1 ")
        );
        let selection = structured(&draft);
        assert!(!draft.restore_launch(&selection, None));
        assert!(draft.permissions_explicit() && draft.wrapper_explicit());
        draft.edit_wrapper("prefix --".into());
        assert!(draft.restore_launch(&selection, None));
        let mut custom = selection.clone();
        custom.permissions = Some(LaunchPermission::Custom);
        custom.permission_flags = Some(vec!["--mode".into(), "ask".into()]);
        assert!(draft.restore_launch(&custom, None));
        assert!(!draft.restore_launch(&custom, None));
        assert!(draft.restore_launch(&selection, None));
        draft.edit_wrapper("'unfinished".into());
        assert!(draft.restore_launch(&selection, None));
        draft.choose_tab(LaunchTab::Command);
        assert!(!draft.edit_prompt("dormant".into()));
        assert_eq!(draft.prompt().as_deref(), Some("dormant"));
    }

    /// Initial destination resolution and post-mint raw reads must describe the
    /// same folder. Exercise actual ownership transitions before relying on the
    /// operational path as the request builder's one ordinary-folder rule.
    #[test]
    fn existing_destination_and_submitted_folder_agree_through_transitions() {
        let assert_linked = |draft: &LaunchDraft| {
            let DestinationDraft::Existing { cwd } = draft.destination() else {
                panic!("premise: the transition selected an existing folder");
            };
            assert_eq!(cwd, draft.submitted_cwd());
        };
        let mut draft = LaunchDraft::new(None);
        assert_linked(&draft);
        let raw = "/work/source\u{1b}";
        draft.seed_folder(raw);
        let display = draft.cwd();
        assert_ne!(display, raw, "premise: the seed requires display escaping");
        assert_linked(&draft);
        assert_eq!(draft.submitted_cwd(), raw);
        draft.choose_destination_type(true);
        draft.edit_repository_query("acme/repo".into());
        draft.select_repository(GithubRepo::parse("acme/repo").unwrap());
        draft.choose_destination_type(false);
        assert_linked(&draft);
        assert_eq!(draft.submitted_cwd(), raw);
        draft.edit_folder(display.clone());
        assert_linked(&draft);
        assert_eq!(
            draft.submitted_cwd(),
            display,
            "a literal edit owns the escaped spelling"
        );
        draft.choose_existing_folder(raw);
        assert_linked(&draft);
        assert_eq!(draft.submitted_cwd(), raw);
        draft.seed_folder(&display);
        assert_linked(&draft);
        let before = draft.clone();
        draft.edit_folder("/different".into());
        draft.restore(before);
        assert_linked(&draft);
    }

    /// Linked placement must preserve raw ownership and dormant editors across
    /// managed mode. Display equality is deliberately insufficient evidence.
    #[test]
    fn placement_keeps_raw_folder_and_explicit_choice_separate() {
        let raw = "/work/peer\u{202e}";
        let mut draft = LaunchDraft::new(None);
        assert_eq!(
            draft.destination(),
            DestinationDraft::Existing { cwd: "~".into() }
        );
        assert_eq!(draft.cwd_raw_seed(), None);
        draft.seed_folder(raw);
        let display = draft.cwd();
        assert_ne!(
            display, raw,
            "premise: the inherited path requires escaping"
        );
        assert_eq!(draft.submitted_cwd(), raw);
        assert!(!draft.cwd_edited() && !draft.folder_is_explicit());
        draft.choose_destination_type(true);
        draft.edit_repository_query("acme/repo".into());
        assert_eq!(
            draft.destination().repo(),
            None,
            "parseable input is still unselected"
        );
        assert!(draft.destination().is_managed());
        let repo = GithubRepo::parse("acme/repo").unwrap();
        draft.select_repository(repo.clone());
        assert_eq!(draft.destination().repo(), Some(&repo));
        assert_eq!(draft.cwd(), display, "the folder editor remains dormant");
        draft.choose_destination_type(false);
        assert_eq!(
            draft.destination(),
            DestinationDraft::Existing { cwd: raw.into() }
        );
        assert_eq!(draft.submitted_cwd(), raw);
        assert!(draft.folder_is_explicit());
        assert!(
            !draft.cwd_edited(),
            "an explicit pick does not claim its display"
        );
        draft.edit_folder(display.clone());
        assert_eq!(draft.submitted_cwd(), display);
        assert_eq!(draft.cwd_raw_seed().as_deref(), Some(raw));
        assert!(draft.cwd_edited());
        draft.choose_existing_folder(raw);
        draft.seed_folder(&display);
        assert_eq!(draft.cwd(), display, "two raw paths can share one display");
        assert_eq!(
            draft.submitted_cwd(),
            display,
            "the latest raw seed owns the path"
        );
        assert_eq!(
            draft.destination(),
            DestinationDraft::Existing { cwd: display }
        );
        assert!(!draft.folder_is_explicit());
    }

    /// Clone defaults, template bases and exact manual names keep their existing
    /// policy. Occupancy freezes for reconciliation, caps at 50 and resets only
    /// its sequence on installation/repository retargeting.
    #[test]
    fn placement_naming_retains_policy_across_retargets_and_edits() {
        use farhelm_proto::launcher::TemplateNameKind;
        let repo = GithubRepo::parse("acme/repo").unwrap();
        let other = GithubRepo::parse("acme/other").unwrap();
        let host = Some(CreateTarget::new(1, "install-a".into()));
        let mut draft = LaunchDraft::new(None);
        draft.seed_title("source\u{202e}", Some(repo.clone()));
        let source_display = draft.title();
        assert_ne!(source_display, "source\u{202e}");
        draft.select_repository(repo.clone());
        draft.retarget_naming(host.clone());
        assert_eq!(draft.effective_title(), "source\u{202e}-clone");
        assert!(!draft.advance_naming(true));
        assert!(draft.advance_naming(false));
        assert_eq!(draft.effective_title(), "source\u{202e}-clone-2");
        draft.retarget_naming(host.clone());
        assert_eq!(draft.effective_title(), "source\u{202e}-clone-2");
        draft.select_repository(other);
        draft.retarget_naming(host.clone());
        assert_eq!(draft.effective_title(), "");
        draft.choose_destination_type(false);
        assert_eq!(
            draft.effective_title(),
            "source\u{202e}",
            "folder restores the copied bytes"
        );
        draft.select_repository(repo);
        draft.retarget_naming(host.clone());
        assert_eq!(draft.effective_title(), "source\u{202e}-clone");
        draft.apply_template_name("review".into(), TemplateNameKind::Fixed);
        assert!(
            draft
                .checkout_naming()
                .default_title(draft.title_edited(), None, draft.destination().repo())
                .is_none()
        );
        assert_eq!(draft.effective_title(), "review");
        draft.apply_template_name("review".into(), TemplateNameKind::Base);
        draft.retarget_naming(host);
        assert_eq!(draft.title_raw_seed(), None);
        assert!(draft.title_edited());
        assert_eq!(
            draft.checkout_naming().template_name(),
            Some(("review", TemplateNameKind::Base))
        );
        for _ in 2..=50 {
            assert!(draft.advance_naming(false));
        }
        assert_eq!(draft.effective_title(), "review-50");
        assert!(
            !draft.advance_naming(false),
            "the bounded sequence keeps its final conflict"
        );
        draft.retarget_naming(Some(CreateTarget::new(1, "install-b".into())));
        assert_eq!(draft.effective_title(), "review");
        assert_eq!(
            draft.checkout_naming().template_name(),
            Some(("review", TemplateNameKind::Base))
        );
        draft.edit_title(source_display.clone());
        assert_eq!(draft.effective_title(), source_display);
        assert_eq!(draft.checkout_naming(), CheckoutNaming::default());
        draft.seed_title("copied", None);
        assert_eq!(
            draft.effective_title(),
            "",
            "Replace has no Clone naming policy"
        );
        draft.edit_title(draft.title());
        assert_eq!(draft.effective_title(), "copied");
        assert_eq!(draft.title_raw_seed().as_deref(), Some("copied"));
    }

    /// Enlarging the owner must not enlarge agent-only reset/restore. Compare
    /// every placement dimension so a whole-value replacement cannot hide here.
    #[test]
    fn agent_resets_and_restore_preserve_placement_and_naming() {
        let mut draft = LaunchDraft::new(None);
        draft.choose_existing_folder("/work/raw\u{1b}");
        draft.seed_title(
            "copied\u{202e}",
            Some(GithubRepo::parse("acme/repo").unwrap()),
        );
        draft.select_repository(GithubRepo::parse("acme/repo").unwrap());
        draft.retarget_naming(Some(CreateTarget::new(1, "install-a".into())));
        assert!(draft.advance_naming(false));
        let before = draft.clone();
        let selection = LaunchSelection {
            harness: LaunchHarness::Codex,
            model: None,
            effort: None,
            permissions: None,
            permission_flags: None,
            wrapper_prefix: None,
            workspace_trust: None,
        };
        for operation in 0..3 {
            match operation {
                0 => draft.reset_choices(Some(LaunchPermission::Guarded)),
                1 => draft.clear_for_command_seed(),
                _ => {
                    draft.restore_launch(&selection, None);
                }
            }
            assert_eq!(draft.cwd(), before.cwd());
            assert_eq!(draft.cwd_raw_seed(), before.cwd_raw_seed());
            assert_eq!(draft.cwd_edited(), before.cwd_edited());
            assert_eq!(draft.folder_is_explicit(), before.folder_is_explicit());
            assert_eq!(draft.title(), before.title());
            assert_eq!(draft.title_raw_seed(), before.title_raw_seed());
            assert_eq!(draft.title_edited(), before.title_edited());
            assert_eq!(draft.destination(), before.destination());
            assert_eq!(draft.checkout_naming(), before.checkout_naming());
        }
        draft.edit_folder("changed".into());
        draft.edit_title("changed".into());
        draft.restore(before.clone());
        assert_eq!(
            draft, before,
            "Undo alone deliberately restores the complete semantic owner"
        );
    }

    /// Use the real envelope builder with fixed external facts. This helper
    /// supplies no alternate field projection or host/preview admission policy.
    fn request(draft: &LaunchDraft) -> IntentBinding {
        draft
            .request(RequestContext {
                host: Some((1, "fixture-incarnation")),
                launch: draft
                    .launch_intent()
                    .expect("the fixture has a complete active intent"),
                replace_source: None,
                checkout: None,
            })
            .expect("the fixture supplies its accepted host fact")
    }

    /// A fixed admitted reply makes changes to draft naming and connection
    /// context distinguishable from the checkout facts that must stay frozen.
    fn accepted_preview() -> GithubPreview {
        GithubPreview {
            canonical_root: "/accepted".into(),
            basename: "checkout".into(),
            cwd: "/accepted/checkout".into(),
            config_revision: 7,
            host: "1".into(),
            incarnation: 4,
            installation_identity: "install-a".into(),
        }
    }

    /// The complete envelope retains raw peer bytes and one-time literal
    /// inputs together. Ordinary blank names share the API's omitted-title
    /// identity, while the editor and every nonblank field retain their bytes.
    #[test]
    fn request_assembles_host_raw_placement_and_structured_one_time_inputs() {
        let mut draft = LaunchDraft::new(None);
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        draft.choose_permissions(Some(LaunchPermission::Custom));
        draft.edit_permission_flags("--mode 'ask me'".into());
        draft.edit_wrapper("prefix 'two words'".into());
        draft.seed_folder("/work/peer\u{202e}");
        draft.seed_title("name\u{202e}", None);
        draft.edit_prompt("--literal\nsecond line".into());
        draft.edit_conversation("literal-conversation".into());
        let binding = draft
            .request(RequestContext {
                host: Some((4, "press-incarnation")),
                launch: draft.launch_intent().unwrap(),
                replace_source: Some("replace-source"),
                checkout: None,
            })
            .unwrap();
        assert_eq!(
            binding,
            IntentBinding {
                host: 4,
                incarnation: "press-incarnation".into(),
                cwd: "/work/peer\u{202e}".into(),
                title: "name\u{202e}".into(),
                prompt: Some("--literal\nsecond line".into()),
                conversation: Some("literal-conversation".into()),
                github_checkout: None,
                agent: LaunchIntent::Structured(LaunchSelection {
                    harness: LaunchHarness::Codex,
                    model: None,
                    effort: None,
                    permissions: Some(LaunchPermission::Custom),
                    permission_flags: Some(vec!["--mode".into(), "ask me".into()]),
                    wrapper_prefix: Some(vec!["prefix".into(), "two words".into()]),
                    workspace_trust: None,
                }),
                replace_source: Some("replace-source".into()),
            }
        );
        draft.apply_template_prompt(Some(String::new()));
        assert_eq!(request(&draft).prompt, Some(String::new()));
        draft.edit_folder(draft.cwd());
        assert_eq!(request(&draft).cwd, "/work/peer<U+202E>");
        draft.edit_title(" \t".into());
        assert_eq!(
            draft.title(),
            " \t",
            "the editor retains the blank spelling"
        );
        assert_eq!(
            request(&draft).title,
            "",
            "ordinary blank titles share the API's omitted-title identity"
        );
        draft.choose_harness(
            LaunchHarness::Pi,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        assert_eq!(
            request(&draft).prompt,
            None,
            "unsupported agents omit the dormant prompt"
        );
        assert_eq!(
            draft.prompt(),
            Some(String::new()),
            "projection does not destroy template presence"
        );
    }

    /// A preliminary binding is not destination admission. Refusing a query
    /// inside the builder would move it ahead of the caller's prompt errors.
    #[test]
    fn request_keeps_unselected_repository_as_a_preliminary_candidate() {
        let mut draft = LaunchDraft::new(None);
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        draft.seed_folder("/dormant/peer\u{1b}");
        draft.edit_repository_query("acme/repo".into());
        assert_eq!(
            draft.destination().repo(),
            None,
            "premise: parseable text is still unselected"
        );
        let binding = request(&draft);
        assert_eq!(binding.cwd, "/dormant/peer\u{1b}");
        assert_eq!(binding.github_checkout, None);
    }

    /// A post-mint structured refresh changes current choices and binding title,
    /// while preserving accepted checkout title/path/configuration and the
    /// original host/source facts. New preview assembly has a different rule.
    #[test]
    fn request_distinguishes_new_preview_from_an_accepted_checkout() {
        let mut draft = LaunchDraft::new(None);
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        draft.edit_title("first-title".into());
        let repo = GithubRepo::parse("acme/repo").unwrap();
        draft.select_repository(repo.clone());
        let preview = accepted_preview();
        let first = draft
            .request(RequestContext {
                host: Some((1, "original-incarnation")),
                launch: draft.launch_intent().unwrap(),
                replace_source: Some("original-source"),
                checkout: Some(CheckoutInput::Preview {
                    repo: &repo,
                    preview: &preview,
                }),
            })
            .unwrap();
        let checkout = first.github_checkout.as_ref().unwrap();
        assert_eq!(first.cwd, preview.cwd.raw());
        assert_eq!(checkout.repo, repo.identifier());
        assert_eq!(checkout.title.as_deref(), Some("first-title"));
        assert_eq!(checkout.preview, preview);
        draft.edit_title(String::new());
        let unnamed = draft
            .request(RequestContext {
                host: Some((first.host, &first.incarnation)),
                launch: first.agent.clone(),
                replace_source: first.replace_source.as_deref(),
                checkout: Some(CheckoutInput::Preview {
                    repo: &repo,
                    preview: &preview,
                }),
            })
            .unwrap();
        assert_eq!(unnamed.title, "");
        assert_eq!(
            unnamed.github_checkout.unwrap().title,
            Some(String::new()),
            "an unnamed preview keeps the existing explicit empty title projection"
        );
        draft.edit_title("new-title".into());
        draft.select_repository(GithubRepo::parse("other/repo").unwrap());
        draft.choose_custom_model("new-model".into(), LaunchHarness::Codex, None);
        draft.edit_prompt("new prompt".into());
        draft.edit_conversation("new-conversation".into());
        // Accepted cwd is an input fact, not a value reconstructed from preview.
        let frozen_cwd = "/accepted/path-spelling";
        let after = draft
            .request(RequestContext {
                host: Some((first.host, &first.incarnation)),
                launch: LaunchIntent::Structured(draft.structured_selection().unwrap()),
                replace_source: first.replace_source.as_deref(),
                checkout: Some(CheckoutInput::Accepted {
                    cwd: frozen_cwd,
                    request: checkout,
                }),
            })
            .unwrap();
        assert_eq!(after.host, first.host);
        assert_eq!(after.incarnation, "original-incarnation");
        assert_eq!(after.replace_source, first.replace_source);
        assert_eq!(after.cwd, frozen_cwd);
        assert_eq!(after.github_checkout, first.github_checkout);
        assert_eq!(after.title, "new-title");
        assert_eq!(after.prompt.as_deref(), Some("new prompt"));
        assert_eq!(after.conversation.as_deref(), Some("new-conversation"));
        let LaunchIntent::Structured(selection) = after.agent else {
            panic!("the ordered recheck supplied Structured");
        };
        assert_eq!(selection.model.as_deref(), Some("new-model"));
    }

    /// Command freezes at the press even if a queued edit changes both tabs.
    /// The envelope must project one-time inputs from that frozen variant, not
    /// from today's selected tab or supported dormant structured harness.
    #[test]
    fn request_keeps_frozen_command_and_accepted_context_after_tab_edits() {
        let mut draft = LaunchDraft::new(None);
        draft.choose_tab(LaunchTab::Command);
        draft.edit_invocation("original-command".into());
        draft.choose_command_yolo(false);
        let frozen = request(&draft);
        let repo = GithubRepo::parse("acme/repo").unwrap();
        let checkout = GithubCheckoutRequest {
            repo: repo.identifier(),
            title: Some("accepted-title".into()),
            preview: accepted_preview(),
        };
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        draft.choose_tab(LaunchTab::Agent);
        draft.edit_invocation("later-command".into());
        draft.choose_command_yolo(true);
        draft.edit_title("current-title".into());
        draft.edit_prompt("current prompt".into());
        draft.edit_conversation("current-conversation".into());
        assert!(
            matches!(draft.launch_intent(), Some(LaunchIntent::Structured(_))),
            "premise: the active draft now describes another variant"
        );
        let after = draft
            .request(RequestContext {
                host: Some((frozen.host, &frozen.incarnation)),
                launch: frozen.agent.clone(),
                replace_source: Some("frozen-source"),
                checkout: Some(CheckoutInput::Accepted {
                    cwd: "/frozen/path",
                    request: &checkout,
                }),
            })
            .unwrap();
        assert_eq!(after.agent, frozen.agent);
        assert_eq!(after.incarnation, frozen.incarnation);
        assert_eq!(after.cwd, "/frozen/path");
        assert_eq!(after.github_checkout, Some(checkout));
        assert_eq!(after.replace_source.as_deref(), Some("frozen-source"));
        assert_eq!(after.title, "current-title");
        assert_eq!((after.prompt, after.conversation), (None, None));
    }

    /// Assertions about agent choices must exercise the active request builder,
    /// rather than bypass tab selection through its normalization helper.
    fn structured(draft: &LaunchDraft) -> LaunchSelection {
        match request(draft).agent {
            LaunchIntent::Structured(selection) => selection,
            LaunchIntent::Command(_) => panic!("the fixture selects the agent tab"),
        }
    }

    /// Inspect the actual command payload without rebuilding its ownership or
    /// hidden-resume rules inside the test.
    fn command(draft: &LaunchDraft) -> CommandLaunch {
        match request(draft).agent {
            LaunchIntent::Command(command) => command,
            LaunchIntent::Structured(_) => panic!("the fixture selects the command tab"),
        }
    }

    /// Preview completion is external observation. It must wake status readers
    /// without rewriting repository intent or subscribing unrelated agent fields
    /// to placement. Live snapshots must also beat memo scheduling within a turn.
    #[farhelm_testtrace::test]
    fn placement_owner_and_external_preview_have_independent_subscriptions() {
        use crate::github_checkout::{GithubPreview, PreviewAuthority, PreviewState};
        use std::cell::Cell;
        std::thread_local! {
            static OWNER: Cell<Option<LaunchDraftState>> = const { Cell::new(None) };
            static PREVIEW: Cell<Option<Signal<PreviewState>>> = const { Cell::new(None) };
            static DEST_RUNS: Cell<usize> = const { Cell::new(0) };
            static NAME_RUNS: Cell<usize> = const { Cell::new(0) };
            static AGENT_RUNS: Cell<usize> = const { Cell::new(0) };
            static PREVIEW_RUNS: Cell<usize> = const { Cell::new(0) };
        }
        /// No render reads: process_events settles the production field memos
        /// and effects without requiring a renderer to acknowledge a new frame.
        fn probe() -> Element {
            let ops = crate::ops::use_op_lock();
            let key = use_signal(|| None);
            let chosen_host = use_signal(|| None);
            let owner = use_launch_draft(
                None,
                Some("/work/peer\u{202e}".into()),
                ops,
                key,
                chosen_host,
            );
            let choices = use_choice_targets();
            let source = use_source_targets(choices);
            use_placement_handles(SystemFixture {
                ordinary: OrdinaryFixture { owner, choices },
                source,
            });
            let fields = owner.fields();
            let preview = use_signal(|| PreviewState::Pending);
            use_effect(move || {
                let _ = (fields.destination_draft)();
                DEST_RUNS.with(|count| count.set(count.get() + 1));
            });
            use_effect(move || {
                let _ = (
                    (fields.title)(),
                    (fields.title_raw_seed)(),
                    (fields.title_edited)(),
                    (fields.clone_checkout_naming)(),
                );
                NAME_RUNS.with(|count| count.set(count.get() + 1));
            });
            use_effect(move || {
                let _ = (fields.structured_harness)();
                AGENT_RUNS.with(|count| count.set(count.get() + 1));
            });
            use_effect(move || {
                let _ = preview();
                PREVIEW_RUNS.with(|count| count.set(count.get() + 1));
            });
            OWNER.with(|slot| slot.set(Some(owner)));
            PREVIEW.with(|slot| slot.set(Some(preview)));
            rsx! {}
        }
        OWNER.with(|slot| slot.set(None));
        PREVIEW.with(|slot| slot.set(None));
        DEST_RUNS.with(|count| count.set(0));
        NAME_RUNS.with(|count| count.set(0));
        AGENT_RUNS.with(|count| count.set(0));
        PREVIEW_RUNS.with(|count| count.set(0));
        let mut dom = VirtualDom::new(probe);
        dom.rebuild_in_place();
        dom.process_events();
        let owner = OWNER.with(Cell::get).unwrap();
        let placement = PLACEMENT_FIXTURE.with(Cell::get).unwrap();
        let mut preview = PREVIEW.with(Cell::get).unwrap();
        let fields = owner.fields();
        assert_eq!(DEST_RUNS.with(Cell::get), 1, "destination observer mounted");
        assert_eq!(NAME_RUNS.with(Cell::get), 1);
        assert_eq!(AGENT_RUNS.with(Cell::get), 1);
        assert_eq!(PREVIEW_RUNS.with(Cell::get), 1);
        let repo = GithubRepo::parse("acme/repo").unwrap();
        dom.in_runtime(|| {
            let display = owner.snapshot().cwd();
            assert_ne!(display, owner.snapshot().submitted_cwd());
            assert!(owner.choose_folder(
                FolderChoice::Recent {
                    cwd: display.clone(),
                    target: Some(CreateTarget::new(1, "install-a".into())),
                },
                None,
                None,
                placement.targets,
                placement.folder
            ));
            assert!(owner.select_repository(repo.clone(), placement.targets, placement.repository));
            apply_fields(
                owner,
                farhelm_proto::launcher::TemplateFields {
                    name: Some("review".into()),
                    name_kind: Some(farhelm_proto::launcher::TemplateNameKind::Base),
                    ..Default::default()
                },
            );
            assert!(
                !fields.destination_draft.peek().is_managed(),
                "premise: destination memo has not settled"
            );
            assert_eq!(fields.title.peek().as_str(), "");
            assert_eq!(owner.snapshot().destination().repo(), Some(&repo));
            assert_eq!(owner.snapshot().submitted_cwd(), display);
            assert_eq!(owner.snapshot().effective_title(), "review");
            dom.in_scope(ScopeId::ROOT, || {
                let target = CreateTarget::new(1, "install-a".into());
                let authority = super::super::BrowseAuthority {
                    target: target.clone(),
                    connection: Some(4),
                    cwd: display.clone(),
                    generation: 1,
                };
                let request = Signal::new(Some(authority.clone()));
                let target = Signal::new(Some(target));
                let connection = Signal::new(Some(4));
                assert!(
                    super::super::browse_activation_is_current(
                        request,
                        &authority,
                        target,
                        connection,
                        &owner.snapshot(),
                    ),
                    "browse activation reads the new raw seed before memos settle"
                );
                let before = owner.snapshot();
                assert!(owner.choose_destination_type(
                    false,
                    placement.targets,
                    placement.repository.open
                ));
                assert!(owner.edit_folder("/other".into(), placement.targets, placement.folder));
                assert!(
                    !super::super::browse_activation_is_current(
                        request,
                        &authority,
                        target,
                        connection,
                        &owner.snapshot(),
                    ),
                    "a same-turn path edit immediately withdraws the old result"
                );
                assert!(owner.choose_folder(
                    FolderChoice::Recent {
                        cwd: before.submitted_cwd(),
                        target: Some(CreateTarget::new(1, "install-a".into())),
                    },
                    None,
                    None,
                    placement.targets,
                    placement.folder
                ));
                assert!(owner.select_repository(
                    repo.clone(),
                    placement.targets,
                    placement.repository
                ));
                assert_eq!(
                    owner.snapshot(),
                    before,
                    "concrete placement choices restore the probe premise"
                );
            });
            let saved = dom.in_scope(ScopeId::ROOT, || {
                super::super::launcher_snapshot(&owner.snapshot(), &[], None)
            });
            assert_eq!(
                saved.destination,
                Some(farhelm_proto::launcher::TemplateDestination::Github(
                    repo.identifier()
                ))
            );
            assert_eq!(saved.name.as_deref(), Some("review"));
        });
        dom.process_events();
        assert_eq!(DEST_RUNS.with(Cell::get), 2);
        assert_eq!(NAME_RUNS.with(Cell::get), 2);
        assert_eq!(AGENT_RUNS.with(Cell::get), 1);
        let semantic_before = dom.in_runtime(|| owner.snapshot());
        let authority = PreviewAuthority {
            generation: 2,
            config_revision: 3,
            host: "1".into(),
            incarnation: 4,
            installation_identity: "install-a".into(),
            repo,
            title: Some("review".into()),
            agent: "fixture-agent".into(),
        };
        let reply = GithubPreview {
            canonical_root: "/checkouts".into(),
            basename: "review".into(),
            cwd: "/checkouts/review".into(),
            config_revision: 3,
            host: "1".into(),
            incarnation: 4,
            installation_identity: "install-a".into(),
        };
        assert!(
            authority.accepts(&authority, &reply),
            "premise: completion is current"
        );
        for state in [
            PreviewState::Ready {
                authority: authority.clone(),
                preview: reply,
            },
            PreviewState::Failed {
                authority,
                message: "occupied".into(),
            },
            PreviewState::Pending,
        ] {
            dom.in_runtime(|| preview.set(state));
            dom.process_events();
            assert_eq!(dom.in_runtime(|| owner.snapshot()), semantic_before);
            assert_eq!(DEST_RUNS.with(Cell::get), 2);
            assert_eq!(NAME_RUNS.with(Cell::get), 2);
            assert_eq!(AGENT_RUNS.with(Cell::get), 1);
        }
        assert_eq!(
            PREVIEW_RUNS.with(Cell::get),
            4,
            "every preview state remains observable"
        );
        dom.in_runtime(|| {
            apply_fields(
                owner,
                farhelm_proto::launcher::TemplateFields {
                    name: Some("review".into()),
                    name_kind: Some(farhelm_proto::launcher::TemplateNameKind::Fixed),
                    ..Default::default()
                },
            )
        });
        dom.process_events();
        assert_eq!(
            NAME_RUNS.with(Cell::get),
            3,
            "same text with a changed naming policy stays observed"
        );
        dom.in_runtime(|| {
            apply_fields(
                owner,
                farhelm_proto::launcher::TemplateFields {
                    name: Some("review".into()),
                    name_kind: Some(farhelm_proto::launcher::TemplateNameKind::Base),
                    ..Default::default()
                },
            )
        });
        dom.process_events();
        assert_eq!(NAME_RUNS.with(Cell::get), 4);
        dom.in_runtime(|| assert!(advance_fixture_name(owner)));
        dom.process_events();
        assert_eq!(
            NAME_RUNS.with(Cell::get),
            5,
            "occupied-name advancement keeps its field trigger"
        );
        assert_eq!(
            dom.in_runtime(|| owner.snapshot().effective_title()),
            "review-2"
        );
        dom.in_runtime(|| owner.edit_title("typed".into()));
        dom.process_events();
        assert_eq!(NAME_RUNS.with(Cell::get), 6, "a hand edit remains observed");
        assert_eq!(DEST_RUNS.with(Cell::get), 2);
        assert_eq!(AGENT_RUNS.with(Cell::get), 1);
        assert_eq!(
            PREVIEW_RUNS.with(Cell::get),
            4,
            "semantic edits do not impersonate preview completion"
        );
        OWNER.with(|slot| slot.set(None));
        PREVIEW.with(|slot| slot.set(None));
    }

    /// Same-turn submissions must see accepted edits before memo tasks run.
    /// Preview observes both tab drafts but excludes flags and one-time inputs;
    /// guarded stamps separately observe raw prompt and conversation edits.
    #[farhelm_testtrace::test]
    fn live_requests_do_not_depend_on_memo_scheduling() {
        use std::cell::Cell;

        std::thread_local! {
            static OWNER: Cell<Option<LaunchDraftState>> = const { Cell::new(None) };
            static CHOICES: Cell<Option<ChoiceTargets>> = const { Cell::new(None) };
            static EFFECT_RUNS: Cell<usize> = const { Cell::new(0) };
            static GUARD_RUNS: Cell<usize> = const { Cell::new(0) };
            static RENDERS: Cell<usize> = const { Cell::new(0) };
        }

        /// Render no draft values, so process_events can drain the actual
        /// production memos and observer without a pending render boundary.
        fn probe() -> Element {
            RENDERS.with(|count| count.set(count.get() + 1));
            let ops = crate::ops::use_op_lock();
            let key = use_signal(|| None);
            let chosen_host = use_signal(|| None);
            let owner = use_launch_draft(None, None, ops, key, chosen_host);
            let choices = use_choice_targets();
            let source = use_source_targets(choices);
            use_placement_handles(SystemFixture {
                ordinary: OrdinaryFixture { owner, choices },
                source,
            });
            let fields = owner.fields();
            use_effect(move || {
                let _ = (
                    (fields.structured_harness)(),
                    (fields.structured_model)(),
                    (fields.structured_effort)(),
                    (fields.structured_permissions)(),
                    (fields.structured_wrapper)(),
                    (fields.launch_tab)(),
                    (fields.invocation)(),
                    (fields.invocation_edited)(),
                    (fields.command_yolo)(),
                    (fields.command_agent)(),
                    (fields.command_resume_on)(),
                    (fields.command_resume)(),
                    (fields.command_resume_edited)(),
                );
                EFFECT_RUNS.with(|count| count.set(count.get() + 1));
            });
            use_effect(move || {
                let _ = ((fields.prompt)(), (fields.conversation)());
                GUARD_RUNS.with(|count| count.set(count.get() + 1));
            });
            OWNER.with(|slot| slot.set(Some(owner)));
            CHOICES.with(|slot| slot.set(Some(choices)));
            rsx! {}
        }

        OWNER.with(|slot| slot.set(None));
        CHOICES.with(|slot| slot.set(None));
        EFFECT_RUNS.with(|count| count.set(0));
        RENDERS.with(|count| count.set(0));
        GUARD_RUNS.with(|count| count.set(0));
        let mut dom = VirtualDom::new(probe);
        dom.rebuild_in_place();
        dom.process_events();
        let owner = OWNER.with(Cell::get).expect("the production owner mounted");
        let choices = CHOICES
            .with(Cell::get)
            .expect("choice history handles mounted");
        let fields = owner.fields();
        assert_eq!(EFFECT_RUNS.with(Cell::get), 1, "the observer subscribed");
        assert_eq!(RENDERS.with(Cell::get), 1);

        dom.in_runtime(|| {
            choose_fixture_harness(owner, LaunchHarness::Codex);
            assert_eq!(
                *fields.structured_harness.peek(),
                None,
                "premise: the memo still holds the pre-event value"
            );
            assert_eq!(structured(&owner.snapshot()).harness, LaunchHarness::Codex);
        });
        dom.process_events();
        assert_eq!(EFFECT_RUNS.with(Cell::get), 2);

        dom.in_runtime(|| {
            owner.choose_permissions(Some(LaunchPermission::Custom), choices);
        });
        dom.process_events();
        assert_eq!(EFFECT_RUNS.with(Cell::get), 3);
        dom.in_runtime(|| {
            owner.edit_permission_flags("--mode 'ask me'".into());
            assert_eq!(fields.structured_permission_flags.peek().as_str(), "");
            assert_eq!(
                structured(&owner.snapshot()).permission_flags,
                Some(vec!["--mode".into(), "ask me".into()])
            );
        });
        dom.process_events();
        dom.in_runtime(|| {
            assert_eq!(
                fields.structured_permission_flags.peek().as_str(),
                "--mode 'ask me'"
            );
        });
        assert_eq!(
            EFFECT_RUNS.with(Cell::get),
            3,
            "flags settled without invalidating the observer"
        );

        dom.in_runtime(|| {
            let context = FINAL_CONTEXT.with(std::cell::Cell::get).unwrap();
            owner.choose_custom_model(
                "private-model".into(),
                LaunchHarness::Codex,
                None,
                choices,
                context.model,
            );
        });
        dom.process_events();
        assert_eq!(EFFECT_RUNS.with(Cell::get), 4, "model remains observed");
        dom.in_runtime(|| {
            owner.choose_effort(Some(LaunchEffort::High), choices);
        });
        dom.process_events();
        assert_eq!(EFFECT_RUNS.with(Cell::get), 5, "effort remains observed");
        dom.in_runtime(|| {
            owner.choose_permissions(Some(LaunchPermission::Guarded), choices);
        });
        dom.process_events();
        assert_eq!(
            EFFECT_RUNS.with(Cell::get),
            6,
            "permission remains observed"
        );
        dom.in_runtime(|| owner.edit_wrapper("nice".into()));
        dom.process_events();
        assert_eq!(EFFECT_RUNS.with(Cell::get), 7, "wrapper remains observed");
        assert_eq!(
            GUARD_RUNS.with(Cell::get),
            1,
            "choice edits do not touch one-time inputs"
        );
        dom.in_runtime(|| {
            owner.choose_command_yolo(true);
            owner.choose_command_agent(Some(LaunchHarness::Claude));
            owner.choose_command_resume(true);
            owner.edit_command_resume("c --resume {conversation} {farhelm_args}".into());
            owner.edit_invocation("c {farhelm_args}".into());
            owner.choose_launch_tab(LaunchTab::Command, choices);
            assert_eq!(*fields.launch_tab.peek(), LaunchTab::Agent);
            assert_eq!(*fields.command_yolo.peek(), None);
            assert_eq!(fields.invocation.peek().as_str(), "");
            assert_eq!(
                command(&owner.snapshot()),
                CommandLaunch {
                    command: "c {farhelm_args}".into(),
                    yolo: true,
                    agent: Some(LaunchHarness::Claude),
                    resume: Some("c --resume {conversation} {farhelm_args}".into()),
                }
            );
            let before = owner.snapshot();
            owner.choose_command_agent(None);
            assert_eq!(command(&owner.snapshot()).resume, None);
            let saved = dom.in_scope(ScopeId::ROOT, || {
                super::super::launcher_snapshot(&owner.snapshot(), &[], None)
            });
            assert_eq!(saved.command_agent, None);
            assert_eq!(
                saved.resume_command,
                before.launch_intent().and_then(|intent| match intent {
                    LaunchIntent::Command(command) => command.resume,
                    LaunchIntent::Structured(_) => None,
                }),
                "templates retain opted-in Resume even when its declaration is hidden"
            );
            owner.choose_command_agent(before.command_agent());
        });
        dom.process_events();
        assert_eq!(EFFECT_RUNS.with(Cell::get), 8);
        dom.in_runtime(|| {
            assert_eq!(*fields.launch_tab.peek(), LaunchTab::Command);
            assert_eq!(*fields.command_yolo.peek(), Some(true));
            assert_eq!(*fields.command_agent.peek(), Some(LaunchHarness::Claude));
            assert!(*fields.command_resume_on.peek());
            assert!(*fields.invocation_edited.peek() && *fields.command_resume_edited.peek());
            assert_eq!(fields.invocation.peek().as_str(), "c {farhelm_args}");
            assert_eq!(
                fields.command_resume.peek().as_str(),
                "c --resume {conversation} {farhelm_args}"
            );
            owner.edit_prompt("first message".into());
            owner.edit_conversation("named-conversation".into());
        });
        dom.process_events();
        assert_eq!(
            EFFECT_RUNS.with(Cell::get),
            8,
            "one-time inputs remain outside preview"
        );
        assert_eq!(
            GUARD_RUNS.with(Cell::get),
            2,
            "one-time edits invalidate guarded stamps"
        );

        // Each edit changes a different observed projection. A whole-draft
        // subscription would also have rerun for the one-time inputs above.
        let edits: [fn(LaunchDraftState, ChoiceTargets); 6] = [
            |owner, choices| {
                owner.choose_launch_tab(LaunchTab::Agent, choices);
            },
            |owner, _| {
                owner.edit_invocation("different command".into());
            },
            |owner, _| {
                owner.choose_command_yolo(false);
            },
            |owner, _| {
                owner.choose_command_agent(None);
            },
            |owner, _| {
                owner.choose_command_resume(false);
            },
            |owner, _| {
                owner.edit_command_resume("different resume".into());
            },
        ];
        for (index, edit) in edits.into_iter().enumerate() {
            dom.in_runtime(|| edit(owner, choices));
            dom.process_events();
            assert_eq!(
                EFFECT_RUNS.with(Cell::get),
                9 + index,
                "preview projection {index} remains observed"
            );
            assert_eq!(
                GUARD_RUNS.with(Cell::get),
                2,
                "choice edit does not invalidate the one-time observer"
            );
        }

        dom.in_runtime(|| {
            owner.choose_launch_tab(LaunchTab::Command, choices);
            seed_invocation_from_source(owner, "c\u{1b}");
        });
        dom.process_events();
        let runs_before_raw_reseed = EFFECT_RUNS.with(Cell::get);
        dom.in_runtime(|| {
            let before = owner.snapshot();
            let display = before.invocation();
            assert_ne!(
                command(&before).command,
                display,
                "premise: the first seed needs escaping"
            );
            seed_invocation_from_source(owner, &display);
            assert_eq!(fields.invocation.peek().as_str(), display);
            assert!(!*fields.invocation_edited.peek());
            assert_eq!(
                command(&owner.snapshot()).command,
                display,
                "a raw-only reseed is visible without waiting for a changed display memo"
            );
            let saved = dom.in_scope(ScopeId::ROOT, || {
                super::super::launcher_snapshot(&owner.snapshot(), &[], None)
            });
            assert_eq!(
                saved.command, display,
                "template snapshots also read the live source bytes"
            );
        });
        dom.process_events();
        assert_eq!(
            EFFECT_RUNS.with(Cell::get),
            runs_before_raw_reseed,
            "raw seeds do not acquire a new preview subscription"
        );

        dom.in_runtime(|| {
            assert_eq!(
                fields.cwd.peek().as_str(),
                "~",
                "premise: placement memos still hold the initial folder"
            );
            assert_eq!(fields.title.peek().as_str(), "");
            let context = FINAL_CONTEXT.with(std::cell::Cell::get).unwrap();
            let mut source = source_prefill();
            source.generation = context.source.prefill_applied.peek().unwrap_or(0) + 1;
            let mut selection = LaunchDraft::new(None);
            selection.choose_harness(
                LaunchHarness::Claude,
                None,
                String::new(),
                ChoiceOrigin::Control,
            );
            source.launch = selection.structured_selection();
            source.command = None;
            source.repo = None;
            source.cwd = "/work/current\u{202e}".into();
            source.title = "current-name\u{202e}".into();
            owner.reconcile_source(Some(&source), false, &[], context.source);
            owner.edit_prompt("same-turn prompt".into());
            owner.edit_conversation("same-turn-conversation".into());
            assert_eq!(fields.cwd.peek().as_str(), "~");
            assert_eq!(fields.title.peek().as_str(), "");
            assert_eq!(
                *fields.structured_harness.peek(),
                Some(LaunchHarness::Codex)
            );
            let binding = request(&owner.snapshot());
            assert_eq!(binding.cwd, "/work/current\u{202e}");
            assert_eq!(binding.title, "current-name\u{202e}");
            assert_eq!(binding.prompt.as_deref(), Some("same-turn prompt"));
            assert_eq!(
                binding.conversation.as_deref(),
                Some("same-turn-conversation")
            );
            assert!(matches!(
                binding.agent,
                LaunchIntent::Structured(LaunchSelection {
                    harness: LaunchHarness::Claude,
                    ..
                })
            ));
        });
        dom.process_events();
        dom.in_runtime(|| {
            assert_eq!(fields.cwd.peek().as_str(), "/work/current<U+202E>");
            assert_eq!(fields.title.peek().as_str(), "current-name<U+202E>");
            assert_eq!(
                *fields.structured_harness.peek(),
                Some(LaunchHarness::Claude)
            );
            assert_eq!(fields.prompt.peek().as_deref(), Some("same-turn prompt"));
            assert_eq!(
                fields.conversation.peek().as_str(),
                "same-turn-conversation"
            );
        });
        assert_eq!(
            RENDERS.with(Cell::get),
            1,
            "the probe never subscribed while rendering"
        );
        OWNER.with(|slot| slot.set(None));
        CHOICES.with(|slot| slot.set(None));
    }

    /// A partial draft must not silently become a launch with a guessed agent.
    #[test]
    fn request_requires_an_agent_and_preserves_raw_editor_text() {
        let mut draft = LaunchDraft::new(None);
        assert_eq!(draft.launch_intent(), None);
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        draft.choose_permissions(Some(LaunchPermission::Custom));
        draft.edit_permission_flags("--sandbox 'unterminated".into());
        draft.edit_wrapper("wrapper 'unterminated".into());
        let request = structured(&draft);
        assert_eq!(request.permission_flags, None);
        assert_eq!(request.wrapper_prefix, None);
        assert_eq!(draft.permission_flags(), "--sandbox 'unterminated");
        assert_eq!(draft.wrapper(), "wrapper 'unterminated");
    }

    /// Direct and search model acceptance deliberately retain different editor
    /// ownership today; template projection must not erase that distinction.
    #[test]
    fn search_model_is_seeded_while_control_model_is_edited() {
        let mut draft = LaunchDraft::new(None);
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        draft.choose_catalog_model(
            "custom".into(),
            LaunchHarness::Codex,
            None,
            String::new(),
            ChoiceOrigin::Search,
        );
        assert_eq!(draft.model_raw_seed(), Some("custom".into()));
        assert!(!draft.model_edited());
        draft.choose_catalog_model(
            "custom".into(),
            LaunchHarness::Codex,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        assert_eq!(draft.model_raw_seed(), None);
        assert!(draft.model_edited());
        assert_eq!(structured(&draft).model.as_deref(), Some("custom"));
    }

    /// Repeated agent choices cannot reconstruct malformed flags or overwrite
    /// an explicitly edited wrapper with remembered defaults.
    #[test]
    fn same_agent_preserves_raw_custom_text_and_wrapper_ownership() {
        let mut draft = LaunchDraft::new(None);
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            "remembered --".into(),
            ChoiceOrigin::Control,
        );
        assert!(!draft.wrapper_explicit());
        draft.choose_permissions(Some(LaunchPermission::Custom));
        draft.edit_permission_flags("--x 'unfinished".into());
        draft.edit_wrapper("mine --".into());
        let outcome = draft.choose_harness(
            LaunchHarness::Codex,
            None,
            "other --".into(),
            ChoiceOrigin::Control,
        );
        assert!(
            outcome.selection_changed,
            "invalid affected flags cannot prove retry equality"
        );
        assert_eq!(draft.permission_flags(), "--x 'unfinished");
        assert_eq!(draft.wrapper(), "mine --");
        assert!(draft.wrapper_explicit());
    }

    /// Reset discards agent-owned words even when remembered permissions survive.
    /// Model-default changes only the model and must preserve independent effort.
    #[test]
    fn default_model_and_reset_keep_their_existing_distinct_rules() {
        let mut draft = LaunchDraft::new(None);
        draft.choose_effort(Some(LaunchEffort::High));
        draft.choose_default_model();
        assert_eq!(draft.effort(), Some(LaunchEffort::High));
        draft.edit_permission_flags("old raw flags".into());
        draft.reset_choices(Some(LaunchPermission::Guarded));
        assert_eq!(draft.harness(), None);
        assert_eq!(draft.effort(), None);
        assert_eq!(draft.permission_flags(), "");
        assert!(!draft.permissions_explicit());
    }

    /// Undo must recover raw values and explicitness rather than reconstructing
    /// them from a normalized request, which cannot represent unfinished text.
    #[test]
    fn snapshot_restore_recovers_raw_values_and_template_ownership() {
        let mut draft = LaunchDraft::new(None);
        draft.edit_wrapper("prefix 'unfinished".into());
        draft.apply_template_permissions(Some(LaunchPermission::Custom), "--a 'b c'".into());
        let before = draft.clone();
        draft.clear_for_command_seed();
        assert!(draft.permissions_explicit());
        assert_eq!(draft.permission_flags(), "");
        draft.restore(before.clone());
        assert_eq!(draft, before);
        assert!(draft.wrapper_explicit());
    }

    /// An unanswered catalog is not evidence that an effort is incompatible.
    /// The same known-empty catalog does establish that fact, both when
    /// choosing a harness and when accepting a typed custom model.
    #[test]
    fn catalog_unknown_and_known_empty_keep_their_distinct_effort_rules() {
        for known in [false, true] {
            let catalog = known.then_some(&[][..]);
            let mut draft = LaunchDraft::new(None);
            draft.choose_harness(
                LaunchHarness::Codex,
                None,
                String::new(),
                ChoiceOrigin::Control,
            );
            draft.choose_effort(Some(LaunchEffort::High));
            assert_eq!(draft.harness(), Some(LaunchHarness::Codex));
            assert_eq!(draft.effort(), Some(LaunchEffort::High));
            let outcome = draft.choose_harness(
                LaunchHarness::Codex,
                catalog,
                String::new(),
                ChoiceOrigin::Control,
            );
            assert_eq!(draft.effort(), (!known).then_some(LaunchEffort::High));
            assert_eq!(outcome.reset_reason.is_some(), known);
            assert_eq!(outcome.selection_changed, known);
            draft.choose_effort(Some(LaunchEffort::High));
            let reason =
                draft.choose_custom_model("private-model".into(), LaunchHarness::Codex, catalog);
            assert_eq!(draft.effort(), (!known).then_some(LaunchEffort::High));
            assert_eq!(reason.reset_reason.is_some(), known);
            assert!(
                reason.selection_changed,
                "accepting a new model changes its submitted bytes"
            );
            assert_eq!(draft.model_owner(), Some(LaunchHarness::Codex));
        }
    }

    /// Template defaults stay raw while the submitted request resolves them.
    /// A wrapper also belongs to its selected agent: a real switch loads the
    /// destination's memory and withdraws the old explicit-template marker.
    #[test]
    fn request_normalizes_defaults_and_agent_switch_loads_its_wrapper() {
        let mut draft = LaunchDraft::new(None);
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            "codex-prefix --".into(),
            ChoiceOrigin::Control,
        );
        draft.apply_template_permissions(None, String::new());
        assert_eq!(draft.permissions(), None);
        assert_eq!(
            structured(&draft).permissions,
            Some(LaunchPermission::Guarded)
        );
        draft.choose_permissions(Some(LaunchPermission::Custom));
        draft.edit_permission_flags("--mode 'ask me'".into());
        draft.edit_wrapper("my-wrapper 'two words'".into());
        let request = structured(&draft);
        assert_eq!(
            request.permission_flags,
            Some(vec!["--mode".into(), "ask me".into()])
        );
        assert_eq!(
            request.wrapper_prefix,
            Some(vec!["my-wrapper".into(), "two words".into()])
        );
        assert_eq!(request.workspace_trust, None);
        draft.choose_harness(
            LaunchHarness::Claude,
            None,
            "claude-prefix --".into(),
            ChoiceOrigin::Control,
        );
        assert_eq!(draft.wrapper(), "claude-prefix --");
        assert!(!draft.wrapper_explicit());
        assert_eq!(draft.permission_flags(), "");
    }

    /// Switching kinds retains each complete draft and one-time inputs. A
    /// command never inherits hidden structured choices or malformed wrappers.
    #[test]
    fn tabs_retain_independent_requests_and_one_time_inputs() {
        let mut draft = LaunchDraft::new(None);
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        draft.choose_custom_model("private-model".into(), LaunchHarness::Codex, None);
        draft.choose_effort(Some(LaunchEffort::High));
        draft.choose_permissions(Some(LaunchPermission::Custom));
        draft.edit_permission_flags("--mode 'ask me'".into());
        draft.edit_wrapper("prefix --".into());
        draft.edit_prompt("first message".into());
        draft.edit_conversation("conversation-id".into());
        let agent = structured(&draft);
        draft.edit_invocation("command {farhelm_args}".into());
        draft.choose_command_yolo(false);
        draft.choose_command_agent(Some(LaunchHarness::Claude));
        draft.apply_template_resume(Some("resume {conversation} {farhelm_args}".into()));
        let both = draft.clone();
        assert!(draft.choose_tab(LaunchTab::Command));
        let command_before = command(&draft);
        assert!(draft.choose_tab(LaunchTab::Agent));
        assert_eq!(structured(&draft), agent);
        assert!(draft.choose_tab(LaunchTab::Command));
        assert_eq!(command(&draft), command_before);
        draft.edit_wrapper("'unfinished".into());
        draft.edit_permission_flags("'unfinished".into());
        assert_eq!(command(&draft), command_before);
        draft.restore(both.clone());
        assert!(
            !draft.choose_tab(LaunchTab::Agent),
            "restoring also restores the active kind"
        );
        assert_eq!(structured(&draft), agent);
        assert_eq!(draft.prompt(), Some("first message".into()));
        assert_eq!(draft.conversation(), "conversation-id");
        assert_eq!(draft, both);
    }

    /// An unanswered assertion is not a non-YOLO answer. Resume is executable
    /// only when both opted in and visible through a declared agent.
    #[test]
    fn command_request_requires_an_answer_and_omits_hidden_resume() {
        let mut draft = LaunchDraft::new(None);
        draft.choose_tab(LaunchTab::Command);
        draft.edit_invocation("c {farhelm_args}".into());
        assert_eq!(draft.launch_intent(), None);
        draft.choose_command_yolo(true);
        draft.choose_command_agent(Some(LaunchHarness::Claude));
        draft.apply_template_resume(Some("c --resume {conversation} {farhelm_args}".into()));
        assert_eq!(
            command(&draft),
            CommandLaunch {
                command: "c {farhelm_args}".into(),
                yolo: true,
                agent: Some(LaunchHarness::Claude),
                resume: Some("c --resume {conversation} {farhelm_args}".into()),
            }
        );
        draft.choose_command_resume(false);
        assert_eq!(command(&draft).resume, None);
        draft.choose_command_resume(true);
        draft.choose_command_agent(None);
        assert_eq!(command(&draft).resume, None);
        assert!(draft.command_resume_on());
        assert_eq!(
            draft.command_resume(),
            "c --resume {conversation} {farhelm_args}"
        );
        draft.edit_invocation(String::new());
        assert_eq!(
            command(&draft).command,
            "",
            "empty-command refusal belongs to submit, before the assertion refusal"
        );
    }

    /// Escaped displays are not reversible encodings. Untouched seeds submit
    /// exact peer bytes; an ordinary edit of the same display claims literal
    /// characters while a template also withdraws the dormant source seed.
    #[test]
    fn command_editor_ownership_preserves_raw_bytes_until_deliberate_edits() {
        let raw = "c\u{1b} {farhelm_args}";
        let raw_resume = "c\n --resume {conversation} {farhelm_args}";
        let mut draft = LaunchDraft::new(None);
        draft.seed_invocation(raw);
        draft.seed_command_fields(Some(&CommandLaunch {
            command: "metadata is not the invocation".into(),
            yolo: false,
            agent: Some(LaunchHarness::Claude),
            resume: Some(raw_resume.into()),
        }));
        let display = draft.invocation();
        let display_resume = draft.command_resume();
        assert_ne!(display, raw, "premise: source text needs display escaping");
        assert_ne!(display_resume, raw_resume);
        assert_eq!(command(&draft).command, raw);
        assert_eq!(command(&draft).resume.as_deref(), Some(raw_resume));
        assert!(!draft.invocation_edited() && !draft.command_resume_edited());
        draft.edit_invocation(display.clone());
        draft.edit_command_resume(display_resume.clone());
        assert_eq!(command(&draft).command, display);
        assert_eq!(command(&draft).resume, Some(display_resume.clone()));
        assert_eq!(draft.invocation_raw_seed().as_deref(), Some(raw));
        assert_eq!(draft.command_resume_raw_seed().as_deref(), Some(raw_resume));
        assert!(draft.invocation_edited() && draft.command_resume_edited());
        draft.apply_template_invocation(display.clone());
        draft.apply_template_resume(Some(display_resume));
        assert_eq!(draft.invocation_raw_seed(), None);
        assert_eq!(draft.command_resume_raw_seed(), None);
        draft.apply_template_resume(None);
        assert!(!draft.command_resume_on());
        assert_eq!(draft.command_resume(), "");
        assert!(draft.command_resume_edited());

        // Two raw seeds can share a display. Neither a display memo nor an
        // edited-bit memo can be the authority for a same-turn source request.
        draft.seed_invocation(raw);
        draft.seed_command_fields(Some(&CommandLaunch {
            command: String::new(),
            yolo: false,
            agent: None,
            resume: None,
        }));
        let escaped = draft.invocation();
        draft.seed_invocation(&escaped);
        assert_eq!(draft.invocation(), escaped);
        assert!(!draft.invocation_edited());
        assert_eq!(command(&draft).command, escaped);
    }

    /// Source reseeding replaces command metadata but clears only the agent
    /// group. A mounted source change clears its prompt and retains the user's
    /// conversation, including when the new source has no resume command.
    #[test]
    fn source_seeding_keeps_new_command_and_mounted_conversation() {
        let mut draft = LaunchDraft::new(Some(LaunchPermission::Guarded));
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            "prefix".into(),
            ChoiceOrigin::Control,
        );
        draft.choose_custom_model("private".into(), LaunchHarness::Codex, None);
        draft.choose_effort(Some(LaunchEffort::High));
        draft.edit_permission_flags("old flags".into());
        draft.edit_prompt("old prompt".into());
        draft.edit_conversation("keep-conversation".into());
        draft.seed_invocation("source command");
        draft.seed_command_fields(Some(&CommandLaunch {
            command: "different metadata command".into(),
            yolo: true,
            agent: Some(LaunchHarness::Claude),
            resume: None,
        }));
        assert_eq!(draft.command_resume_raw_seed(), Some(String::new()));
        assert_eq!(
            draft.command_resume(),
            "(empty)",
            "source displays retain the peer empty marker"
        );
        draft.clear_prompt_for_prefill();
        let command_before = command(&draft);
        draft.clear_for_command_seed();
        assert_eq!(draft.tab(), LaunchTab::Command);
        assert_eq!(command(&draft), command_before);
        assert_eq!(command(&draft).command, "source command");
        assert_eq!(draft.conversation(), "keep-conversation");
        assert_eq!(draft.prompt(), None);
        assert_eq!(draft.harness(), None);
        assert_eq!(draft.model(), None);
        assert_eq!(draft.model_owner(), None);
        assert_eq!(draft.effort(), None);
        assert_eq!(draft.permissions(), None);
        assert!(draft.permissions_explicit());
        assert_eq!(draft.permission_flags(), "");
        assert_eq!(draft.wrapper(), "");
        assert!(!draft.wrapper_explicit());

        draft.seed_command_fields(None);
        assert_eq!(draft.command_resume_raw_seed(), None);
        assert_eq!(draft.command_resume(), "");
        assert!(!draft.command_resume_edited());
        assert_eq!(draft.command_yolo(), None);
        assert_eq!(draft.command_agent(), None);
        assert!(!draft.command_resume_on());
        assert_eq!(draft.invocation(), "source command");
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        draft.choose_tab(LaunchTab::Agent);
        assert_eq!(structured(&draft).harness, LaunchHarness::Codex);
        assert_eq!(draft.conversation(), "keep-conversation");

        let retained = (draft.invocation(), draft.prompt(), draft.conversation());
        draft.edit_permission_flags("discarded flags".into());
        draft.reset_choices(None);
        assert_eq!(
            (draft.invocation(), draft.prompt(), draft.conversation()),
            retained
        );
        assert_eq!(
            draft.permission_flags(),
            "",
            "Reset keeps the earlier flag-clear fix"
        );
    }

    /// Prompt presence belongs to the edit source. Projection may omit a
    /// dormant prompt without destroying it, while templates keep Some("").
    #[test]
    fn prompt_edits_and_template_presence_keep_distinct_contracts() {
        use farhelm_proto::launcher::LauncherKind;
        let mut draft = LaunchDraft::new(None);
        draft.edit_prompt(String::new());
        assert_eq!(draft.prompt(), None);
        draft.apply_template_prompt(Some(String::new()));
        assert_eq!(draft.prompt(), Some(String::new()));
        assert_eq!(
            super::super::active_prompt(LauncherKind::Agent, draft.harness(), &draft.prompt()),
            Some(String::new())
        );
        draft.edit_prompt("message".into());
        draft.choose_harness(
            LaunchHarness::Pi,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        assert_eq!(
            super::super::active_prompt(LauncherKind::Agent, draft.harness(), &draft.prompt()),
            None
        );
        draft.choose_tab(LaunchTab::Command);
        assert_eq!(
            super::super::active_prompt(LauncherKind::Command, draft.harness(), &draft.prompt()),
            None
        );
        assert_eq!(draft.prompt(), Some("message".into()));
    }

    /// Undo is a complete value restore, including dormant text and seed
    /// ownership that no normalized executable request can reconstruct.
    #[test]
    fn undo_restores_both_tabs_one_time_inputs_and_raw_ownership() {
        let mut draft = LaunchDraft::new(Some(LaunchPermission::Guarded));
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            "prefix".into(),
            ChoiceOrigin::Control,
        );
        draft.choose_custom_model("private-model".into(), LaunchHarness::Codex, None);
        draft.choose_effort(Some(LaunchEffort::High));
        draft.choose_permissions(Some(LaunchPermission::Custom));
        draft.edit_permission_flags("--mode 'ask me'".into());
        draft.seed_invocation("source\u{1b}");
        draft.seed_command_fields(Some(&CommandLaunch {
            command: "source".into(),
            yolo: true,
            agent: Some(LaunchHarness::Claude),
            resume: Some("resume\n".into()),
        }));
        draft.apply_template_prompt(Some(String::new()));
        draft.edit_conversation("conversation".into());
        let before = draft.clone();
        draft.choose_tab(LaunchTab::Agent);
        draft.choose_harness(
            LaunchHarness::Claude,
            None,
            String::new(),
            ChoiceOrigin::Control,
        );
        draft.apply_template_invocation("edited".into());
        draft.apply_template_resume(None);
        draft.choose_command_yolo(false);
        draft.choose_command_agent(None);
        draft.edit_prompt("changed prompt".into());
        draft.edit_conversation("changed conversation".into());
        assert_ne!(draft, before, "premise: every owned group has changed");
        draft.restore(before.clone());
        assert_eq!(draft, before);
        assert_eq!(draft.launch_intent(), before.launch_intent());
    }
}

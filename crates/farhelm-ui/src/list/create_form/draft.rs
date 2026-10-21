//! Accepted tab drafts and one-time inputs, independent of picker widgets.
//!
//! Raw editor text and ownership belong together: parsing a draft and writing
//! it back would lose unfinished quotes and change untouched peer values.
//! Host authority, attempt keys, confirmation and transient picker state stay
//! with the form. `LaunchDraftState` is only the renderer's ownership boundary.

use dioxus::prelude::*;

use crate::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, api};

use super::{
    CommandLaunch, LaunchIntent, LaunchTab, command_intent, display_peer, submitted_field,
};

/// Accepted launch choices and their raw-text ownership, retained across tabs.
///
/// This is a draft, not a validated request: malformed custom flags and wrapper
/// text must remain available for correction. The request projection does not
/// replace the form's ordered admission checks. Destination fields still belong
/// to the form while that independent group is migrated. Both tab drafts and
/// one-time inputs survive mode changes; only the active intent is submitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LaunchDraft {
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
}

/// Preserve the existing control/search differences while sharing transitions.
/// Search restores model ownership like a relayed choice; the model control
/// accepts deliberately edited text. Their compatibility notices also differ.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ChoiceOrigin {
    Control,
    Search,
}

/// A compatibility notice and the direct harness control's retry comparison.
/// The caller additionally checks whether its active launch tab changed.
pub(super) struct AgentChoiceOutcome {
    pub(super) reset_reason: Option<String>,
    pub(super) selection_changed: bool,
}

impl LaunchDraft {
    /// Seed only remembered permissions; New never guesses an agent or model.
    pub(super) fn new(remembered_permission: Option<LaunchPermission>) -> Self {
        Self {
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

    /// Tab changes select a request variant without copying or clearing drafts.
    /// The returned fact lets each caller keep its existing retry-key rule.
    pub(super) fn choose_tab(&mut self, tab: LaunchTab) -> bool {
        let changed = self.tab != tab;
        self.tab = tab;
        changed
    }

    /// Ordinary typing claims the displayed spelling but retains its old seed.
    pub(super) fn edit_invocation(&mut self, text: String) {
        self.invocation = text;
        self.invocation_edited = true;
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
    pub(super) fn choose_command_yolo(&mut self, yolo: bool) {
        self.command_yolo = Some(yolo);
    }

    /// Clearing a declaration hides Resume without destroying its retained draft.
    pub(super) fn choose_command_agent(&mut self, agent: Option<LaunchHarness>) {
        self.command_agent = agent;
    }

    /// Resume visibility changes independently of its retained text ownership.
    pub(super) fn choose_command_resume(&mut self, enabled: bool) {
        self.command_resume_on = enabled;
    }

    /// Ordinary resume typing claims display text without discarding its seed.
    pub(super) fn edit_command_resume(&mut self, text: String) {
        self.command_resume = text;
        self.command_resume_edited = true;
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
    pub(super) fn edit_prompt(&mut self, text: String) {
        self.prompt = (!text.is_empty()).then_some(text);
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
    pub(super) fn edit_conversation(&mut self, text: String) {
        self.conversation = text;
    }

    /// Project the active intent, before its host/destination envelope exists.
    ///
    /// Missing harness means no structured request; an unanswered YOLO assertion
    /// means no command request. Empty commands still project: submit owns their
    /// refusal and retry exemption, before the unanswered-assertion refusal.
    /// Malformed wrapper text is omitted because render-time retry matching has always
    /// been lenient; actual submission must first enforce its wrapper guard.
    /// Custom flags likewise retain the existing caller-owned validation order.
    pub(super) fn request(&self) -> Option<LaunchIntent> {
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

    /// Reuse structured normalization after the post-mint missing-agent and
    /// wrapper guards, before that caller refuses a changed active mode.
    pub(super) fn structured_selection(&self) -> Option<LaunchSelection> {
        let mut selection = self.editing_selection(self.harness?);
        selection.permissions = crate::launch_composer::normalized_permissions(
            selection.harness,
            selection.permissions,
        );
        Some(selection)
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
        let outcome = AgentChoiceOutcome {
            reset_reason: super::draft_reconciliation_reason(
                &before,
                &selection,
                self.permissions_explicit,
            ),
            selection_changed: old_harness != Some(selection.harness) || selection != before,
        };
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
        outcome
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
            let offered = catalog
                .unwrap_or_default()
                .iter()
                .find(|model| model.id == id && model.harness == harness)
                .map(|model| model.efforts.as_slice())
                .unwrap_or_default();
            let reset_reason = self.effort.filter(|effort| !offered.contains(effort)).map(|_| {
                self.effort = None;
                "the selected effort is not in Farhelm's offering for that model, so it was cleared".to_string()
            });
            self.model = Some(id);
            self.model_raw_seed = None;
            self.model_edited = true;
            self.model_owner = None;
            return AgentChoiceOutcome {
                reset_reason,
                selection_changed: true,
            };
        }
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
            selection_changed: true,
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
    ) -> Option<String> {
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
        reason
    }

    /// Choosing the default model does not erase an independently chosen effort.
    pub(super) fn choose_default_model(&mut self) {
        self.model = None;
        self.model_raw_seed = None;
        self.model_edited = false;
        self.model_owner = None;
    }

    /// Report a real effort change so repeated choices can remain retries.
    pub(super) fn choose_effort(&mut self, effort: Option<LaunchEffort>) -> bool {
        let changed = self.effort != effort;
        self.effort = effort;
        changed
    }

    /// A deliberate mode choice owns permissions and clears unrelated flags.
    /// Custom mode retains its raw buffer so returning to its editor loses no text.
    pub(super) fn choose_permissions(&mut self, permission: Option<LaunchPermission>) -> bool {
        let changed = self.permissions != permission;
        self.permissions = permission;
        if permission != Some(LaunchPermission::Custom) {
            self.permission_flags.clear();
        }
        self.permissions_explicit = true;
        changed
    }

    /// Keep the exact editor buffer even when it is not yet a valid argument list.
    pub(super) fn edit_permission_flags(&mut self, text: String) {
        self.permission_flags = text;
        self.permissions_explicit = true;
    }

    /// Editing a remembered prefix makes it an explicit reusable choice.
    pub(super) fn edit_wrapper(&mut self, text: String) {
        self.wrapper = text;
        self.wrapper_explicit = true;
    }

    /// Reapply New's passive permission seed while preserving launch context.
    /// Clearing the selected agent also ends its ownership of custom words;
    /// leaving them dormant would let search restore them under another CLI.
    /// Command, tab and one-time values remain exactly as they were.
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

    /// A stored launch replaces the whole structured choice and is explicit.
    /// The caller supplies model-owner evidence: Clone has source ownership;
    /// a recent uses the current catalog's custom_model_owner decision.
    pub(super) fn restore_launch(
        &mut self,
        selection: &LaunchSelection,
        owner: Option<LaunchHarness>,
    ) {
        self.accept_reconciled(selection, owner, false);
        self.permissions = crate::launch_composer::normalized_permissions(
            selection.harness,
            selection.permissions,
        );
        self.permission_flags = joined(selection.permission_flags.as_ref());
        self.wrapper = joined(selection.wrapper_prefix.as_ref());
        self.permissions_explicit = true;
        self.wrapper_explicit = true;
    }

    /// A command/legacy prefill has no hidden structured choice to inherit.
    /// Clear only that group: the caller has already seeded its new command,
    /// and mounted source changes preserve the one-time conversation selector.
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
        self.permissions = permission;
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

/// The only mutable renderer handle for the accepted draft.
/// The form still owns admission and retry policy during the staged migration;
/// edit() is the single place that will receive those existing policies.
#[derive(Clone, Copy)]
pub(super) struct LaunchDraftState {
    value: Signal<LaunchDraft>,
    fields: DraftFields,
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
    /// Every accepted transition writes through this one ownership boundary.
    pub(super) fn edit<R>(mut self, edit: impl FnOnce(&mut LaunchDraft) -> R) -> R {
        edit(&mut self.value.write())
    }
}

/// Mount one draft and equality-filtered projections of its accepted fields.
/// Only these memos subscribe to the aggregate; effects subscribe to the
/// individual values they already observed, not every unrelated edit.
pub(super) fn use_launch_draft(permission: Option<LaunchPermission>) -> LaunchDraftState {
    let value = use_signal(|| LaunchDraft::new(permission));
    let fields = DraftFields {
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
    LaunchDraftState { value, fields }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assertions about agent choices must exercise the active request builder,
    /// rather than bypass tab selection through its normalization helper.
    fn structured(draft: &LaunchDraft) -> LaunchSelection {
        match draft
            .request()
            .expect("the fixture has a complete active intent")
        {
            LaunchIntent::Structured(selection) => selection,
            LaunchIntent::Command(_) => panic!("the fixture selects the agent tab"),
        }
    }

    /// Inspect the actual command payload without rebuilding its ownership or
    /// hidden-resume rules inside the test.
    fn command(draft: &LaunchDraft) -> CommandLaunch {
        match draft
            .request()
            .expect("the fixture has an answered command assertion")
        {
            LaunchIntent::Command(command) => command,
            LaunchIntent::Structured(_) => panic!("the fixture selects the command tab"),
        }
    }

    /// Supply unrelated placement/UI handles to the real template adapter.
    /// The owner under test still supplies every moved field; reconstructing
    /// those fields here would hide a stale-memo or hidden-resume regression.
    /// Call inside the DOM's root scope so it owns these temporary handles.
    fn template_targets(draft: LaunchDraftState) -> super::super::TemplateTargets {
        super::super::TemplateTargets {
            draft,
            composer_reset_reason: Signal::new(None),
            chosen_host: Signal::new(None),
            destination: Signal::new(crate::github_checkout::DestinationDraft::Existing {
                cwd: "/workspace".into(),
            }),
            cwd: Signal::new("/workspace".into()),
            cwd_raw_seed: Signal::new(None),
            cwd_edited: Signal::new(false),
            title: Signal::new(String::new()),
            title_raw_seed: Signal::new(None),
            title_edited: Signal::new(false),
            clone_checkout_naming: Signal::new(super::super::CheckoutNaming::default()),
            preview_revision: Signal::new(0),
            template_error: Signal::new(None),
            intent_key: Signal::new(None),
        }
    }

    /// Same-turn submissions must see accepted edits before memo tasks run.
    /// Preview observes both tab drafts but excludes flags and one-time inputs;
    /// guarded stamps separately observe raw prompt and conversation edits.
    #[farhelm_testtrace::test]
    fn live_requests_do_not_depend_on_memo_scheduling() {
        use std::cell::Cell;

        std::thread_local! {
            static OWNER: Cell<Option<LaunchDraftState>> = const { Cell::new(None) };
            static EFFECT_RUNS: Cell<usize> = const { Cell::new(0) };
            static GUARD_RUNS: Cell<usize> = const { Cell::new(0) };
            static RENDERS: Cell<usize> = const { Cell::new(0) };
        }

        /// Render no draft values, so process_events can drain the actual
        /// production memos and observer without a pending render boundary.
        fn probe() -> Element {
            RENDERS.with(|count| count.set(count.get() + 1));
            let owner = use_launch_draft(None);
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
            rsx! {}
        }

        OWNER.with(|slot| slot.set(None));
        EFFECT_RUNS.with(|count| count.set(0));
        RENDERS.with(|count| count.set(0));
        GUARD_RUNS.with(|count| count.set(0));
        let mut dom = VirtualDom::new(probe);
        dom.rebuild_in_place();
        dom.process_events();
        let owner = OWNER.with(Cell::get).expect("the production owner mounted");
        let fields = owner.fields();
        assert_eq!(EFFECT_RUNS.with(Cell::get), 1, "the observer subscribed");
        assert_eq!(RENDERS.with(Cell::get), 1);

        dom.in_runtime(|| {
            owner.edit(|draft| {
                draft.choose_harness(
                    LaunchHarness::Codex,
                    None,
                    String::new(),
                    ChoiceOrigin::Control,
                );
            });
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
            owner.edit(|draft| draft.choose_permissions(Some(LaunchPermission::Custom)));
        });
        dom.process_events();
        assert_eq!(EFFECT_RUNS.with(Cell::get), 3);
        dom.in_runtime(|| {
            owner.edit(|draft| draft.edit_permission_flags("--mode 'ask me'".into()));
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
            owner.edit(|draft| {
                draft.choose_custom_model("private-model".into(), LaunchHarness::Codex, None);
            });
        });
        dom.process_events();
        assert_eq!(EFFECT_RUNS.with(Cell::get), 4, "model remains observed");
        dom.in_runtime(|| {
            owner.edit(|draft| draft.choose_effort(Some(LaunchEffort::High)));
        });
        dom.process_events();
        assert_eq!(EFFECT_RUNS.with(Cell::get), 5, "effort remains observed");
        dom.in_runtime(|| {
            owner.edit(|draft| draft.choose_permissions(Some(LaunchPermission::Guarded)));
        });
        dom.process_events();
        assert_eq!(
            EFFECT_RUNS.with(Cell::get),
            6,
            "permission remains observed"
        );
        dom.in_runtime(|| owner.edit(|draft| draft.edit_wrapper("nice".into())));
        dom.process_events();
        assert_eq!(EFFECT_RUNS.with(Cell::get), 7, "wrapper remains observed");
        assert_eq!(
            GUARD_RUNS.with(Cell::get),
            1,
            "choice edits do not touch one-time inputs"
        );
        dom.in_runtime(|| {
            owner.edit(|draft| {
                draft.choose_command_yolo(true);
                draft.choose_command_agent(Some(LaunchHarness::Claude));
                draft.choose_command_resume(true);
                draft.edit_command_resume("c --resume {conversation} {farhelm_args}".into());
                draft.edit_invocation("c {farhelm_args}".into());
                draft.choose_tab(LaunchTab::Command);
            });
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
            owner.edit(|draft| draft.choose_command_agent(None));
            assert_eq!(command(&owner.snapshot()).resume, None);
            let saved = dom.in_scope(ScopeId::ROOT, || {
                super::super::launcher_snapshot(template_targets(owner), &[], None)
            });
            assert_eq!(saved.command_agent, None);
            assert_eq!(
                saved.resume_command,
                before.request().and_then(|intent| match intent {
                    LaunchIntent::Command(command) => command.resume,
                    LaunchIntent::Structured(_) => None,
                }),
                "templates retain opted-in Resume even when its declaration is hidden"
            );
            owner.edit(|draft| draft.restore(before));
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
            owner.edit(|draft| {
                draft.edit_prompt("first message".into());
                draft.edit_conversation("named-conversation".into());
            });
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
        let edits: [fn(&mut LaunchDraft); 6] = [
            |draft| {
                draft.choose_tab(LaunchTab::Agent);
            },
            |draft| draft.edit_invocation("different command".into()),
            |draft| draft.choose_command_yolo(false),
            |draft| draft.choose_command_agent(None),
            |draft| draft.choose_command_resume(false),
            |draft| draft.edit_command_resume("different resume".into()),
        ];
        for (index, edit) in edits.into_iter().enumerate() {
            dom.in_runtime(|| owner.edit(edit));
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
            owner.edit(|draft| {
                draft.choose_tab(LaunchTab::Command);
                draft.seed_invocation("c\u{1b}");
            });
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
            owner.edit(|draft| draft.seed_invocation(&display));
            assert_eq!(fields.invocation.peek().as_str(), display);
            assert!(!*fields.invocation_edited.peek());
            assert_eq!(
                command(&owner.snapshot()).command,
                display,
                "a raw-only reseed is visible without waiting for a changed display memo"
            );
            let saved = dom.in_scope(ScopeId::ROOT, || {
                super::super::launcher_snapshot(template_targets(owner), &[], None)
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

        assert_eq!(
            RENDERS.with(Cell::get),
            1,
            "the probe never subscribed while rendering"
        );
        OWNER.with(|slot| slot.set(None));
    }

    /// A partial draft must not silently become a launch with a guessed agent.
    #[test]
    fn request_requires_an_agent_and_preserves_raw_editor_text() {
        let mut draft = LaunchDraft::new(None);
        assert_eq!(draft.request(), None);
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
        draft.choose_harness(
            LaunchHarness::Codex,
            None,
            "other --".into(),
            ChoiceOrigin::Control,
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
            draft.choose_effort(Some(LaunchEffort::High));
            let reason =
                draft.choose_custom_model("private-model".into(), LaunchHarness::Codex, catalog);
            assert_eq!(draft.effort(), (!known).then_some(LaunchEffort::High));
            assert_eq!(reason.is_some(), known);
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
        assert_eq!(draft.request(), None);
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
        assert_eq!(draft.request(), before.request());
    }
}

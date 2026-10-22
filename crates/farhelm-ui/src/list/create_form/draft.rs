//! The launcher's accepted agent choices, independent of its picker widgets.
//!
//! Raw editor text and ownership belong together: parsing a draft and writing
//! it back would lose unfinished quotes and change untouched peer values.
//! Host authority, attempt keys, confirmation and transient picker state stay
//! with the form. `LaunchDraftState` is only the renderer's ownership boundary.

use dioxus::prelude::*;

use crate::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, api};

/// Accepted agent choices and their raw-text ownership, retained across tabs.
///
/// This is a draft, not a validated request: malformed custom flags and wrapper
/// text must remain available for correction. The request projection does not
/// replace the form's ordered admission checks. Command and destination fields
/// still belong to the form while those independent groups are migrated.
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

    /// Snapshot submission identically before minting, after minting and on retry.
    ///
    /// Missing harness means no structured request. Malformed wrapper text is
    /// omitted in this projection because render-time retry matching has always
    /// been lenient; actual submission must first enforce its wrapper guard.
    /// Custom flags likewise retain the existing caller-owned validation order.
    pub(super) fn request(&self) -> Option<LaunchSelection> {
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
    /// The dormant flag buffer is retained here to preserve the existing reset
    /// behavior; changing that ownership rule belongs to a separate fix.
    pub(super) fn reset_choices(&mut self, permission: Option<LaunchPermission>) {
        self.harness = None;
        self.choose_default_model();
        self.effort = None;
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
    pub(super) fn clear_for_command_seed(&mut self) {
        *self = Self::new(None);
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

    /// Same-turn submissions must see accepted edits before memo tasks run.
    /// Once those tasks settle, flags alone must not invalidate observers of
    /// the agent choices used by the existing preview fingerprint.
    #[farhelm_testtrace::test]
    fn live_requests_do_not_depend_on_memo_scheduling() {
        use std::cell::Cell;

        std::thread_local! {
            static OWNER: Cell<Option<LaunchDraftState>> = const { Cell::new(None) };
            static EFFECT_RUNS: Cell<usize> = const { Cell::new(0) };
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
                );
                EFFECT_RUNS.with(|count| count.set(count.get() + 1));
            });
            OWNER.with(|slot| slot.set(Some(owner)));
            rsx! {}
        }

        OWNER.with(|slot| slot.set(None));
        EFFECT_RUNS.with(|count| count.set(0));
        RENDERS.with(|count| count.set(0));
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
            assert_eq!(
                owner.snapshot().request().unwrap().harness,
                LaunchHarness::Codex
            );
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
                owner.snapshot().request().unwrap().permission_flags,
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
        let request = draft
            .request()
            .expect("the selected agent supplies a snapshot");
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
        assert_eq!(draft.request().unwrap().model.as_deref(), Some("custom"));
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

    /// Reset and model-default have separate contracts. Preserve the dormant
    /// flag buffer until its aligned fix, while effort survives model-default.
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
        assert_eq!(draft.permission_flags(), "old raw flags");
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
            draft.request().unwrap().permissions,
            Some(LaunchPermission::Guarded)
        );
        draft.choose_permissions(Some(LaunchPermission::Custom));
        draft.edit_permission_flags("--mode 'ask me'".into());
        draft.edit_wrapper("my-wrapper 'two words'".into());
        let request = draft.request().unwrap();
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
}

//! The template editor: the right-hand pane of the Templates dialog, where one template is opened, edited and saved.
//!
//! SPEC.md, "Launch templates" (the editor sentences): "The editor shows only fields the template sets, with labeled
//! controls and explicit add/remove actions"; "Saving is explicit; Save is greyed out and does nothing while there are
//! no unsaved changes, and keeps keyboard focus after saving". Ported from the old app's `FieldEditor`, `Choice`,
//! `NameKindControl` and the editor half of `TemplatesDialog` (`list/templates.rs` at reference commit `24bc0580`),
//! with the `.templates-editor` rules of its stylesheet. The rules a draft follows (which fields exist, what adding and
//! removing does, why a draft cannot be saved) are [`super::fields`]'s; this module draws a draft and turns the user's
//! input into it.
//!
//! Here's the TLDR of the rules:
//!
//! - **What it holds.** The stored template's name it opened (`previous`, none for a new template), the draft as it
//!   was opened or last saved (`baseline`), and the draft being edited. "Unsaved changes" is the two differing. A
//!   stored template opens as [`Draft::from_template`] against its raw stored fields, so a kindless template's inferred
//!   switch and a dropped workspace-trust field show as unsaved. The leave question asks only about edits the user made
//!   since opening or saving that Save would store, so those automatic changes alone never interrupt a departure.
//! - **The draft is the template as loaded.** It does not follow a change pushed while it is open: the save writes the
//!   whole template, last write wins (SPEC.md, "One GUI at a time", confirmed 2026-10-21), and the editor never takes
//!   a pushed value into a field. The text fields are plain inputs over the draft, as the launcher's are; the drafts
//!   foundation's bound field ([`crate::drafts::DraftInput`]) follows a push only when its editor hands it one, and a
//!   whole-template save is not its one-field save, so nothing here would make it follow.
//! - **Text fields.** One input per text field the draft sets, made when the field appears and dropped when it goes,
//!   so removing a field and adding it again starts from its starting value (SPEC.md: "removing and adding the field
//!   replaces it"). A field whose stored text holds characters an input cannot show or keep ([`Field::read_only`]) is
//!   shown escaped and read-only, and the raw value goes on being saved untouched; the escaped form never reaches the
//!   draft.
//! - **Saving.** Save is the one write: `PUT /api/templates/{name}` with the whole template, as work on the template's
//!   key in the operation store. It refuses first, in the old order: a new or changed name with whitespace, the helm's
//!   own shape check, the draft's own reason ([`Draft::refusal`]), then a name another template has
//!   ([`name_refusal`]). A refusal or a failed save is said in the dialog's status line under the panes, where the old
//!   dialog said it; a failed save keeps the draft for the user to correct and is never retried. While the save is in
//!   flight Save is busy (it keeps focus and ignores presses), every input is read-only, and every other edit is
//!   refused. Once it succeeds, the saved draft is the baseline.
//! - **Names are read from the store.** The templates the helm pushed are the list a new name is checked against.
//!   A change this window makes is pushed before the reply to the command that made it (`ui_channel::run` in the helm
//!   crate sends a command's changes, then its reply), so the list already holds a template just saved, which closes
//!   the old dialog's gap where a reload could still lack it. While the helm reports the collection unavailable, or
//!   before its first snapshot, a new name cannot be proven free and is refused.
//! - **Renaming.** Saving a stored template under a changed name saves it under the new name, gives the new name the
//!   ⚡ mark if the old one held it, moves the old name's number key to the new name, then deletes the old name (SPEC.md:
//!   "a rename saves that draft under the new name and then deletes the old one"; "Renaming repoints the mark before
//!   deleting the old name"; "Rename keeps a key"). Each step is its own write, on its own key, started when the step
//!   before it has settled; a step that fails stops the rename with the old dialog's words for it, and the old template
//!   stays, so nothing is lost; the copy under the new name stays too, and saving the draft again is refused as a taken
//!   name, as in the old dialog (the user opens the new name, or renames the draft back). The whole rename counts as one
//!   save for Save's busy state and the dialog's.
//! - **Duplicate** (stored templates only) opens an unsaved copy of the template as it was opened or last saved, not of
//!   unsaved edits, named by [`duplicate_name`] over the stored names, as a new template that needs Save.
//! - **Delete** (stored templates only) sends `DELETE /api/templates/{name}` at once, unsaved edits or not (SPEC.md:
//!   "Delete takes effect immediately"). Its label is decided from the stored template when the footer is drawn: one
//!   that fails today's save rules says "Delete permanently", since Undo restores through the ordinary save, which the
//!   helm would refuse. The press carries out the kind its label said ([`DeleteKind`]). Once the helm has deleted it,
//!   the dialog drops the editor and offers Undo ([`Ended::Deleted`]); a refusal is said in the status line.

use std::collections::HashMap;

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::launcher::{
    LaunchCatalogModel, LaunchTemplate, TemplateDestination, TemplateFields,
    check_new_template_name, check_template_shape, quick_launch_eligible,
};
use farhelm_proto::ui_channel::{Availability, Collection, HostView};
use farhelm_proto::{LaunchPermission, LaunchTarget};
use gpui::{
    AnyElement, AnyWindowHandle, App, Context, FocusHandle, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Role, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, WeakEntity, Window, div, px,
};

use super::dialog::TemplatesDialog;
use super::fields::{Draft, Field, duplicate_name, name_refusal};
use super::shortcuts::{self, Bindings};
use crate::components::{
    Button, HoverHelp, TextInputEvent, TextInputState, Tier, capitalize_validation,
};
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText};
use crate::launcher::draft::RunsOn;
use crate::launcher::request::Recipes;
use crate::operations::{Command, OpKey, Outcome, Work};
use crate::session;
use crate::theme::{self, Palette, metrics};

mod rows;

/// The session name's two kinds explained, the old editor's words, under the session name and in its kind control's
/// hover help.
pub(super) const NAME_KIND_HELP: &str = "Base name: used as-is when free, otherwise numbered (app-review-2, -3, …). \
                                         Fixed name: used exactly; a clash is reported.";

/// `.templates-editor`: the gap between the editor's blocks.
const EDITOR_GAP: f32 = 14.;
/// `.templates-editor-heading h3`: the heading's size.
const HEADING_TEXT: f32 = 15.;
/// `.templates-hint`, `.templates-error`: 12-pixel text at a 1.5 line height.
pub(super) const SMALL_LINE_HEIGHT: f32 = 18.;
/// `.templates-field`: the gap between a field's label and its control.
pub(super) const FIELD_GAP: f32 = 6.;
/// `.templates-add-menu`: the gap between its entries, its padding and its offset under "+ Add field".
const ADD_MENU_GAP: f32 = 4.;
const ADD_MENU_PAD: f32 = 8.;
/// The initial prompt starts four rows high, as the old `<textarea rows=4>` did.
const PROMPT_ROWS: usize = 4;

/// The fields the editor shows as a text input of their own. The others are choices, drawn without one.
const TEXT_FIELDS: [Field; 7] = [
    Field::Command,
    Field::Model,
    Field::Wrapper,
    Field::Resume,
    Field::Prompt,
    Field::Destination,
    Field::Name,
];

/// Which text input an edit came from: the template's own name, one field's input, or the Custom permission words
/// under Approvals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Text {
    TemplateName,
    Field(Field),
    Flags,
}

/// One text input and the subscription that mirrors its edits into the draft; dropping it drops both.
struct Input {
    state: TextInputState,
    _events: Subscription,
}

/// The id of the sentence in the editor's status line that the editor's own refusals are said in (the line around it
/// is the dialog's `templates-editor-status`). A template name's refusal brings its own sentence,
/// `template-name-refusal`, built in `fields.rs`.
const STATUS: &str = "templates-editor-status-text";

/// A refusal in the app's own words, for the status line (the dialog's own refusals of an Undo too).
pub(super) fn said(words: impl Into<SharedString>) -> HostSentence {
    HostSentence::new(STATUS).text(words)
}

/// The helm's refusal, in its own words, which may quote host values, for the status line.
pub(super) fn helm_said(message: &HostText) -> HostSentence {
    HostSentence::new(STATUS).host(message)
}

/// The write the editor has in flight. While one is, the editor takes no edits and the dialog no other write.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Writing {
    /// The save of the draft under `name`: a new template's, or a stored one's under its own name.
    Save { name: String },
    /// A stored template saved under a new name: `step` is the step in flight ([`Editor::rename_from`]).
    Rename {
        old: String,
        new: String,
        step: Step,
    },
    /// The stored template's Delete: `undo` is the copy to offer Undo for once it is deleted (none for "Delete
    /// permanently"), `mark` the user's press, which focus moves for.
    Delete {
        undo: Option<Box<LaunchTemplate>>,
        mark: Mark,
    },
}

/// How one of the editor's writes ended, for the dialog to act on what reaches beyond the editor: a departure that
/// waited for a save, or the editor's own end once its template is deleted.
pub(super) enum Ended {
    /// Not yet: a rename's next step is in flight.
    Going,
    /// The draft is saved (a rename's every step included).
    Saved,
    /// Refused or stopped; the status line says why, and the draft is kept.
    Stopped,
    /// The stored template is deleted: Undo is offered for `undo`, if the label said it would be, and focus moves for
    /// the press at `mark`.
    Deleted {
        undo: Option<Box<LaunchTemplate>>,
        mark: Mark,
    },
}

/// Which Delete the footer offers for the stored template, decided when it is drawn: an ordinary one, which offers
/// Undo, or "Delete permanently" for a stored template that fails today's save rules, whose Undo the helm would refuse.
/// The press carries out the kind its label said (SPEC.md: "The label the user saw decides"), never one worked out
/// again when the press is handled: a GPUI handler belongs to the frame that drew it, so it holds the drawn kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeleteKind {
    Restorable,
    Permanent,
}

/// A rename's steps, in order: the template saved under its new name, the ⚡ mark moved, the number key moved, the old
/// name deleted. The two moves happen only when the old name held the mark or a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Put,
    Mark,
    Keys,
    Delete,
}

impl Step {
    /// The step after this one, or `None` after the last.
    fn next(self) -> Option<Step> {
        match self {
            Step::Put => Some(Step::Mark),
            Step::Mark => Some(Step::Keys),
            Step::Keys => Some(Step::Delete),
            Step::Delete => None,
        }
    }

    /// What the status line says when this step is refused: the helm's words for the save under the new name and for
    /// the deletion of the old one, the old dialog's own words for the moves, which say the old template was kept.
    fn refused(self, message: &HostText) -> HostSentence {
        match self {
            Step::Put | Step::Delete => helm_said(message),
            Step::Mark => said(
                "The new name was saved, but the quick launch mark could not be moved; the old template was kept",
            ),
            Step::Keys => said(
                "The new name was saved, but its number key could not be moved; the old template was kept",
            ),
        }
    }
}

/// One open template. See the module docs.
pub(super) struct Editor {
    /// The stored template's name this editor opened or last saved, or `None` for a new template.
    previous: Option<String>,
    /// The raw stored template, or an empty template before the first save. Differences enable Save.
    baseline: Draft,
    /// What the editor showed on opening or just after Save. Leaving asks only when the draft differs from this and
    /// from `baseline` ([`Editor::needs_leave_confirmation`]), so an inferred switch or removed legacy trust field never
    /// asks on its own.
    leave_baseline: Draft,
    /// The template being edited.
    draft: Draft,
    /// The last stored definition and its delete label. Draft edits redraw the
    /// footer but need not reparse the stored definition's command placeholders.
    stored_delete: Option<(LaunchTemplate, DeleteKind)>,
    /// The text inputs of the fields the draft sets, and of its name. The Custom permission words live only in theirs,
    /// as typed: the draft holds them parsed, or none while they do not parse, and an input is put back to the draft's
    /// words only when its own text would not parse to them, so a half-typed quote stays as typed.
    inputs: HashMap<Text, Input>,
    /// Whether the "+ Add field" menu is open.
    add_open: bool,
    /// "+ Add field"'s focus handle, where focus returns when Escape closes its menu.
    add_focus: FocusHandle,
    /// Save's focus handle, so it keeps focus through a save.
    save_focus: FocusHandle,
    /// What the dialog's status line says for the editor, until the next save or until another template opens.
    status: Option<HostSentence>,
    writing: Option<Writing>,
    /// The window the editor is drawn in: a write settles outside any window's update, and what follows it (the
    /// departure a save waited for, focus after a delete) needs the window.
    window: AnyWindowHandle,
}

impl Editor {
    /// An editor on the stored `template`: the draft is [`Draft::from_template`] (the switch its fields imply, no
    /// workspace trust), the baseline its raw stored fields, as the old editor compared them.
    pub(super) fn open(
        template: &LaunchTemplate,
        window: &mut Window,
        cx: &mut Context<TemplatesDialog>,
    ) -> Editor {
        let baseline = Draft {
            name: template.name.clone(),
            fields: template.fields.clone(),
            wrapper_input: None,
        };
        Editor::with(
            Some(template.name.clone()),
            baseline,
            Draft::from_template(template),
            window,
            cx,
        )
    }

    /// An editor on a new template: no name, the Agent switch, no fields (the old editor's new draft).
    pub(super) fn new_template(window: &mut Window, cx: &mut Context<TemplatesDialog>) -> Editor {
        Editor::with(None, Draft::default(), Draft::default(), window, cx)
    }

    /// The one constructor behind the others: `previous`, `baseline` and `draft` as their docs say, and an input for the
    /// name and for each text field the draft sets, the Custom words' input spelling the draft's stored words (joined
    /// so they parse back to the same words).
    fn with(
        previous: Option<String>,
        baseline: Draft,
        draft: Draft,
        window: &mut Window,
        cx: &mut Context<TemplatesDialog>,
    ) -> Editor {
        let mut editor = Editor {
            leave_baseline: if previous.is_some() {
                draft.clone()
            } else {
                baseline.clone()
            },
            previous,
            baseline,
            draft,
            stored_delete: None,
            inputs: HashMap::new(),
            add_open: false,
            add_focus: cx.focus_handle().tab_stop(true),
            save_focus: cx.focus_handle().tab_stop(true),
            status: None,
            writing: None,
            window: window.window_handle(),
        };
        editor.sync_inputs(window, cx);
        editor
    }

    /// The stored template this editor holds, by name: the list's row drawn pressed.
    pub(super) fn previous(&self) -> Option<&str> {
        self.previous.as_deref()
    }

    /// Whether a write of this editor's is in flight.
    pub(super) fn is_writing(&self) -> bool {
        self.writing.is_some()
    }

    /// What the dialog's status line says for the editor.
    pub(super) fn status(&self) -> Option<&HostSentence> {
        self.status.as_ref()
    }

    /// The name field's focus handle, where focus goes when a template opens.
    pub(super) fn name_focus(&self, cx: &App) -> Option<FocusHandle> {
        self.inputs
            .get(&Text::TemplateName)
            .map(|input| input.state.focus_handle(cx))
    }

    /// Forget that the "+ Add field" menu is open, moving no focus: the header Close does this before it closes the
    /// dialog, since only Escape closes the menu on its own.
    pub(super) fn forget_add_menu(&mut self) {
        self.add_open = false;
    }

    /// Duplicate: an editor on an unsaved copy of the stored template this one holds, as it was opened or last saved
    /// (never its unsaved edits), with the switch inferred and workspace trust dropped as opening does, named by
    /// [`duplicate_name`] over `names`, the stored names. The copy is a new template, so it differs from its empty
    /// baseline and needs Save. `None` for a new template, which has nothing stored to copy.
    pub(super) fn duplicate(
        &self,
        names: &[&str],
        window: &mut Window,
        cx: &mut Context<TemplatesDialog>,
    ) -> Option<Editor> {
        self.previous.as_ref()?;
        let mut copy = Draft::from_template(&self.baseline.template());
        copy.name = duplicate_name(&copy.name, names);
        Some(Editor::with(None, Draft::default(), copy, window, cx))
    }

    /// Escape while the "+ Add field" menu is open closes the menu first and puts focus back on "+ Add field", as the
    /// old dialog's Escape did. Returns whether the menu was open, so the dialog stays.
    pub(super) fn close_add_menu(&mut self, window: &mut Window, cx: &mut App) -> bool {
        if !self.add_open {
            return false;
        }
        self.add_open = false;
        focus::focus_in_layer(&self.add_focus, &Mark::take(cx), window, cx);
        true
    }

    /// The draft being edited, for tests that check what it holds.
    #[cfg(test)]
    pub(super) fn draft(&self) -> &Draft {
        &self.draft
    }

    /// The Custom words field's text, typed or loaded (empty without the field), for tests that check the field shows
    /// its own template's words and keeps a half-typed quote.
    #[cfg(test)]
    pub(super) fn flags_text(&self, cx: &App) -> String {
        self.typed_text(Text::Flags, cx)
    }

    /// What the input `which` holds now, or nothing when the draft has no such input.
    fn typed_text(&self, which: Text, cx: &App) -> String {
        self.inputs
            .get(&which)
            .map(|input| input.state.text(cx).to_string())
            .unwrap_or_default()
    }

    /// Replace the draft's name, as typing it would, for the screenshot golden that shows the leave question.
    #[cfg(test)]
    pub(super) fn rename_draft(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<TemplatesDialog>,
    ) {
        self.edit(window, cx, |editor| editor.draft.name = name.to_owned());
    }

    /// Whether Save would change the stored template, including automatic upgrades to legacy fields.
    pub(super) fn dirty(&self) -> bool {
        self.draft != self.baseline
    }

    /// Whether leaving would lose something the user did: the draft differs from what the editor showed on opening or
    /// after the last save, and Save would store it. Farhelm's own fill-ins on opening (the inferred switch, the dropped
    /// workspace-trust field) differ only from the stored template, and an edit back to the stored template differs only
    /// from what opened; neither asks, since leaving keeps the stored template as it is. A new or duplicated draft's
    /// `leave_baseline` is its empty baseline, so for those this is [`Editor::dirty`].
    pub(super) fn needs_leave_confirmation(&self) -> bool {
        self.dirty() && self.draft != self.leave_baseline
    }

    /// Which Delete the footer offers ([`DeleteKind`]), judged on the stored template, not the draft: Delete removes
    /// the stored value even when unsaved edits have repaired it, and Undo would have to save that value back. A name
    /// the store no longer holds reads as an ordinary Delete, whose press then refuses on its own.
    fn delete_kind(&mut self, cx: &App) -> DeleteKind {
        let Some(stored) = self.stored(cx) else {
            return DeleteKind::Restorable;
        };
        if let Some((checked, kind)) = &self.stored_delete
            && checked == &stored
        {
            return *kind;
        }
        let kind = if check_template_shape(&stored).is_ok() {
            DeleteKind::Restorable
        } else {
            DeleteKind::Permanent
        };
        self.stored_delete = Some((stored, kind));
        kind
    }

    /// The stored template this editor holds, as the store has it now.
    fn stored(&self, cx: &App) -> Option<LaunchTemplate> {
        let name = self.previous.as_deref()?;
        let store = session::current_store(cx)?;
        store
            .read(cx)
            .collections()
            .get::<LaunchTemplate>(name)
            .cloned()
    }

    // ---- Edits ------------------------------------------------------------------------------------------------------

    /// The user changed the text input `which`: mirror it into the draft. A read-only input takes no edit, and while a
    /// write is in flight none does; then the input is put back to what the draft holds (an edit queued before the
    /// input became read-only, as the drafts foundation guards against too).
    fn typed(&mut self, which: Text, window: &mut Window, cx: &mut Context<TemplatesDialog>) {
        let Some(text) = self
            .inputs
            .get(&which)
            .map(|input| input.state.text(cx).to_string())
        else {
            return;
        };
        if self.writing.is_some() || self.read_only(which) {
            self.sync_inputs(window, cx);
            return;
        }
        apply_text(&mut self.draft, which, text);
        cx.notify();
    }

    /// Make an edit that is not typing (a choice, an added or removed field, a suggestion), unless a write is in
    /// flight, and bring the inputs in step with the draft afterwards: an input appears or goes with its field, and
    /// one whose value the edit replaced shows the new value.
    fn edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<TemplatesDialog>,
        change: impl FnOnce(&mut Editor),
    ) {
        if self.writing.is_some() {
            return;
        }
        change(self);
        self.sync_inputs(window, cx);
        cx.notify();
    }

    /// Choose the "Switches launcher to" value. It never clears a field: one that no longer fits stays visible and
    /// refuses saving (SPEC.md), and the add menu closes, as the old control closed it.
    fn choose_kind(
        &mut self,
        kind: Option<farhelm_proto::launcher::LauncherKind>,
        window: &mut Window,
        cx: &mut Context<TemplatesDialog>,
    ) {
        self.edit(window, cx, |editor| {
            editor.draft.fields.kind = kind;
            editor.add_open = false;
        });
    }

    /// Add `field` with its starting value ([`Field::add`]) and close the add menu.
    fn add(&mut self, field: Field, window: &mut Window, cx: &mut Context<TemplatesDialog>) {
        self.edit(window, cx, |editor| {
            field.add(&mut editor.draft.fields);
            editor.add_open = false;
        });
    }

    /// Remove `field` ([`Field::remove`]), with the wrapper prefix's unparsed text; the Custom words' input goes with
    /// the Approvals that own it.
    fn remove(&mut self, field: Field, window: &mut Window, cx: &mut Context<TemplatesDialog>) {
        self.edit(window, cx, |editor| {
            field.remove(&mut editor.draft.fields);
            if field == Field::Wrapper {
                editor.draft.wrapper_input = None;
            }
        });
    }

    /// Choose the Approvals value. Custom takes the words typed under it, or, with no words field yet (Custom was not
    /// chosen), the words the draft still holds: a stored template that fails today's save rules can keep words beside
    /// another choice, and the old editor's seeded words brought them back on Custom. Any other choice drops them, as
    /// the old editor's `permission_words` did, so a template never stores words its choice does not use.
    fn choose_permissions(
        &mut self,
        permission: Option<LaunchPermission>,
        window: &mut Window,
        cx: &mut Context<TemplatesDialog>,
    ) {
        let typed = match self.inputs.get(&Text::Flags) {
            Some(input) => input.state.text(cx).to_string(),
            None => self.shown(Text::Flags),
        };
        self.edit(window, cx, |editor| {
            editor.draft.fields.permissions = Some(permission);
            editor.draft.fields.permission_flags = (permission == Some(LaunchPermission::Custom))
                .then(|| farhelm_proto::split_permission_flags(&typed).ok())
                .flatten();
        });
    }

    /// Choose where the template's sessions run, from the Host field's Runs on switch: `target`'s new environment, or
    /// with `None` an existing host, which starts unchosen. Either way the legacy recipe-only field goes, so the template
    /// is stored in the canonical shape. The segment already chosen changes nothing, which keeps a recipe, place or host
    /// the user chose there. The old editor's `on_target`, with its `PlacementPicker`'s rule for the chosen segment.
    fn choose_runs_on(
        &mut self,
        target: Option<LaunchTarget>,
        window: &mut Window,
        cx: &mut Context<TemplatesDialog>,
    ) {
        let current = self.draft.fields.launch_target();
        if RunsOn::of(target.as_ref()) == RunsOn::of(current.as_ref()) {
            return;
        }
        self.edit(window, cx, |editor| {
            let fields = &mut editor.draft.fields;
            fields.host = target.is_none().then(String::new);
            fields.target = target;
            fields.recipe = None;
        });
    }

    /// Change one part of the template's environment, `change` given the environment the template names (its target,
    /// or a legacy recipe read as one), which is then stored as its target with the legacy recipe gone. Nothing
    /// happens without an environment.
    fn edit_target(
        &mut self,
        window: &mut Window,
        cx: &mut Context<TemplatesDialog>,
        change: impl FnOnce(LaunchTarget) -> LaunchTarget,
    ) {
        let Some(target) = self.draft.fields.launch_target() else {
            return;
        };
        self.edit(window, cx, |editor| {
            editor.draft.fields.target = Some(change(target));
            editor.draft.fields.recipe = None;
        });
    }

    /// The name field's "Replace name": the name is cleared, so the field becomes editable, and focus goes into it.
    fn replace_name(&mut self, window: &mut Window, cx: &mut Context<TemplatesDialog>) {
        if self.writing.is_some() {
            return;
        }
        let mark = Mark::take(cx);
        self.edit(window, cx, |editor| editor.draft.name.clear());
        if let Some(name) = self.name_focus(cx) {
            focus::focus_in_layer(&name, &mark, window, cx);
        }
    }

    // ---- Inputs -----------------------------------------------------------------------------------------------------

    /// Whether the input `which` shows its value escaped and read-only ([`Field::read_only`]; the template's name by
    /// the same rule).
    fn read_only(&self, which: Text) -> bool {
        match which {
            Text::TemplateName => self
                .draft
                .name
                .chars()
                .any(farhelm_proto::text::is_presentation_unsafe),
            Text::Field(field) => field.read_only(&self.draft.fields),
            Text::Flags => false,
        }
    }

    /// What the input `which` shows for the draft: the value as typed, or escaped when it is read-only.
    fn shown(&self, which: Text) -> String {
        let fields = &self.draft.fields;
        let value = match which {
            Text::TemplateName => self.draft.name.clone(),
            Text::Flags => {
                return fields
                    .permission_flags
                    .as_deref()
                    .map(farhelm_proto::join_permission_flags)
                    .unwrap_or_default();
            }
            Text::Field(Field::Command) => fields.command.clone().unwrap_or_default(),
            Text::Field(Field::Model) => fields.model.clone().flatten().unwrap_or_default(),
            Text::Field(Field::Resume) => {
                fields.resume_command.clone().flatten().unwrap_or_default()
            }
            Text::Field(Field::Prompt) => fields.prompt.clone().unwrap_or_default(),
            Text::Field(Field::Name) => fields.name.clone().unwrap_or_default(),
            Text::Field(Field::Destination) => match &fields.destination {
                Some(TemplateDestination::Folder(text) | TemplateDestination::Github(text)) => {
                    text.clone()
                }
                None => String::new(),
            },
            // Text that does not parse yet was typed here, so it holds nothing an input cannot show: never escaped.
            Text::Field(Field::Wrapper) => match &self.draft.wrapper_input {
                Some(text) => return text.clone(),
                None => fields
                    .wrapper_prefix
                    .clone()
                    .flatten()
                    .map(|words| words.join(" "))
                    .unwrap_or_default(),
            },
            Text::Field(
                Field::Yolo
                | Field::Agent
                | Field::Effort
                | Field::Permissions
                | Field::Container
                | Field::Host,
            ) => String::new(),
        };
        if self.read_only(which) {
            farhelm_proto::text::escaped(&value)
        } else {
            value
        }
    }

    /// Bring the inputs in step with the draft: make the input of each text field the draft sets (and of the Custom
    /// words while Custom is chosen), drop the others, and put the draft's value into an input only where the input's
    /// own text would not give the draft that value, so the caret and spacing of what the user is typing stay put.
    /// Every input is read-only while it shows an escaped value or while a write is in flight.
    fn sync_inputs(&mut self, window: &mut Window, cx: &mut Context<TemplatesDialog>) {
        let wanted: Vec<Text> = std::iter::once(Text::TemplateName)
            .chain(
                TEXT_FIELDS
                    .into_iter()
                    .filter(|field| field.is_set(&self.draft.fields))
                    .map(Text::Field),
            )
            .chain(
                (self.draft.fields.permissions == Some(Some(LaunchPermission::Custom)))
                    .then_some(Text::Flags),
            )
            .collect();
        self.inputs.retain(|which, _| wanted.contains(which));
        for which in wanted {
            if let std::collections::hash_map::Entry::Vacant(slot) = self.inputs.entry(which) {
                slot.insert(make_input(which, &self.draft.fields, window, cx));
            }
            let read_only = self.read_only(which);
            let shown = self.shown(which);
            let Some(input) = self.inputs.get(&which) else {
                continue;
            };
            let text = input.state.text(cx).to_string();
            let in_step = if read_only {
                text == shown
            } else {
                let mut probe = self.draft.clone();
                apply_text(&mut probe, which, text.clone());
                probe == self.draft
            };
            if !in_step {
                input.state.set_text(shown, window, cx);
            }
            input
                .state
                .set_read_only(read_only || self.writing.is_some(), cx);
            // The destination's placeholder follows its kind, which a choice beside it switches.
            if which == Text::Field(Field::Destination)
                && let Some(placeholder) = spec(which, &self.draft.fields).placeholder
            {
                input.state.set_placeholder(placeholder, window, cx);
            }
        }
    }

    // ---- Saving -----------------------------------------------------------------------------------------------------

    /// Why the draft cannot be saved now, in the old editor's order, or `None`. `catalog` is the dialog's model catalog,
    /// which judges the effort; `None` (unread, or a failed read) refuses no effort.
    fn refusal(&self, catalog: Option<&[LaunchCatalogModel]>, cx: &App) -> Option<HostSentence> {
        let name = &self.draft.name;
        let renamed = self.previous.as_deref() != Some(name.as_str());
        if renamed && let Err(reason) = check_new_template_name(name) {
            return Some(said(capitalize_validation(reason)));
        }
        if let Err(reason) = check_template_shape(&self.draft.template()) {
            return Some(said(capitalize_validation(reason)));
        }
        if let Some(reason) = self.draft.refusal(catalog) {
            return Some(said(reason));
        }
        let names = stored_names(cx);
        let names: Option<Vec<&str>> = names
            .as_ref()
            .map(|names| names.iter().map(String::as_str).collect());
        name_refusal(self.previous.as_deref(), name, names.as_deref())
    }

    /// Save: with unsaved changes and no write in flight, check the draft and write it whole. A stored template saved
    /// under another name is renamed ([`Editor::rename_from`] takes it from there). See the module docs. Returns
    /// whether the write was sent, so a departure can wait for it.
    pub(super) fn save(
        &mut self,
        catalog: Option<&[LaunchCatalogModel]>,
        window: &mut Window,
        cx: &mut Context<TemplatesDialog>,
    ) -> bool {
        if self.writing.is_some() || !self.dirty() {
            return false;
        }
        if let Some(reason) = self.refusal(catalog, cx) {
            self.status = Some(reason);
            cx.notify();
            return false;
        }
        let template = self.draft.template();
        let Some(command) = save_command(&template) else {
            self.status = Some(said("This template cannot be sent to Farhelm as it stands"));
            cx.notify();
            return false;
        };
        let writing = match self.previous.clone() {
            Some(old) if old != template.name => Writing::Rename {
                old,
                new: template.name.clone(),
                step: Step::Put,
            },
            _ => Writing::Save {
                name: template.name.clone(),
            },
        };
        self.status = None;
        let sent = self.start(&OpKey::template(&template.name), command, writing, cx);
        if sent {
            self.sync_inputs(window, cx);
        }
        cx.notify();
        sent
    }

    /// Delete the stored template this editor holds, as the footer's `kind` of Delete said. The stored template as the
    /// store holds it now is the copy Undo restores (SPEC.md, "One GUI at a time": "Undo of a delete restores the copy
    /// the Templates dialog had loaded rather than the one deleted"). See the module docs.
    pub(super) fn delete(
        &mut self,
        kind: DeleteKind,
        window: &mut Window,
        cx: &mut Context<TemplatesDialog>,
    ) {
        if self.writing.is_some() {
            return;
        }
        let Some(name) = self.previous.clone() else {
            return;
        };
        let Some(stored) = self.stored(cx) else {
            self.status = Some(said(
                "This template is no longer stored, so there is nothing to delete",
            ));
            cx.notify();
            return;
        };
        let Some(command) = delete_command(&name) else {
            return;
        };
        let writing = Writing::Delete {
            undo: (kind == DeleteKind::Restorable).then(|| Box::new(stored)),
            mark: Mark::take(cx),
        };
        self.status = None;
        if self.start(&OpKey::template(&name), command, writing, cx) {
            self.sync_inputs(window, cx);
        }
        cx.notify();
    }

    /// Send `command` on `key` as the editor's write `writing`, its completion reaching the dialog
    /// ([`TemplatesDialog::write_settled`]) inside the editor's window. Returns whether it was sent; the operation store
    /// refuses a key with work in flight, which the editor's own busy state keeps from happening, and a retired store
    /// sends nothing.
    fn start(
        &mut self,
        key: &OpKey,
        command: Command,
        writing: Writing,
        cx: &mut Context<TemplatesDialog>,
    ) -> bool {
        let Some(operations) = session::current_operations(cx) else {
            return false;
        };
        let weak = cx.weak_entity();
        let window = self.window;
        let preference = *key == OpKey::preferences();
        let started = operations.update(cx, |operations, cx| {
            operations.start(key, command, cx, move |settled, cx| {
                let outcome = settled.outcome.clone();
                // A completion runs from the store's own task, never inside a window's update, so the window is
                // free to enter. A closed window has closed the dialog with it.
                let _ = window.update(cx, |_, window, cx| {
                    weak.update(cx, |dialog, cx| {
                        // A rename's preference writes are the editor's: the line under the panes that reports a
                        // refused ⚡ or number-key write from the list must not take them for its own. Writes on a
                        // template's key leave that line alone.
                        if preference {
                            dialog.forget_preference_write();
                        }
                        dialog.write_settled(outcome, window, cx);
                        cx.notify();
                    })
                });
            })
        });
        match started {
            Ok(()) => {
                self.writing = Some(writing);
                true
            }
            Err(not_started) => {
                tracing::debug!(?not_started, "template write not started");
                false
            }
        }
    }

    /// The editor's write in flight settled. A save that succeeded makes the draft as sent the new baseline and the
    /// stored template this editor holds; a refused one puts the helm's words in the status line and keeps the draft
    /// for correction. A rename goes on to its next step, or stops with its step's words, the old template kept. Once
    /// nothing is in flight the inputs take edits again, and focus stays where it is (on Save, which kept it). A
    /// deletion that succeeded is handed to the dialog, which drops this editor.
    pub(super) fn settled(&mut self, outcome: Outcome, cx: &mut Context<TemplatesDialog>) -> Ended {
        let Some(writing) = self.writing.take() else {
            return Ended::Stopped;
        };
        // The template route answers a write at once and never accepts one as long work (202); the preference route
        // neither. Both mean done.
        let refused = match outcome {
            Outcome::Done(_) | Outcome::Accepted(_) => None,
            Outcome::Refused(refusal) => Some(refusal.words),
        };
        let ended = match (writing, refused) {
            (Writing::Save { name }, None) => {
                self.finish_save(name);
                Ended::Saved
            }
            (Writing::Rename { old, new, step }, None) => match step.next() {
                Some(next) => {
                    self.rename_from(old, new, next, cx);
                    if self.writing.is_some() {
                        Ended::Going
                    } else {
                        Ended::Stopped
                    }
                }
                None => {
                    self.finish_save(new);
                    Ended::Saved
                }
            },
            (Writing::Delete { undo, mark }, None) => return Ended::Deleted { undo, mark },
            (Writing::Save { .. } | Writing::Delete { .. }, Some(message)) => {
                self.status = Some(helm_said(&message));
                Ended::Stopped
            }
            (Writing::Rename { step, .. }, Some(message)) => {
                self.status = Some(step.refused(&message));
                Ended::Stopped
            }
        };
        if self.writing.is_none() {
            for (which, input) in &self.inputs {
                input.state.set_read_only(self.read_only(*which), cx);
            }
        }
        ended
    }

    /// A save, or the last step of a rename, succeeded: the draft as sent is the baseline, and `name` the stored
    /// template the editor holds.
    fn finish_save(&mut self, name: String) {
        self.baseline = self.draft.clone();
        self.leave_baseline = self.draft.clone();
        self.previous = Some(name);
    }

    /// Go on with the rename of `old` to `new` (already saved under `new`) from the first step at or after `step` that
    /// has something to do, in the old dialog's order: give `new` the ⚡ mark if `old` holds it, move `old`'s number key
    /// to `new` (the whole map, written as the list writes it), then delete `old` (SPEC.md: "Renaming repoints the mark
    /// before deleting the old name"; "Rename keeps a key"). Each step is read from the store as it stands when the step
    /// before it settles, which already holds that step's result, and each is work on its own key. A step that cannot
    /// go on stops the rename with its own words, and the old template stays.
    fn rename_from(
        &mut self,
        old: String,
        new: String,
        step: Step,
        cx: &mut Context<TemplatesDialog>,
    ) {
        // Without the bindings the rename cannot tell whether the old name holds ⚡ or a key, and moving on would delete
        // the old name and with it its mark and key (the old dialog's "shortcut bindings could not be read").
        let bindings = match Bindings::read(cx) {
            Ok(bindings) => bindings,
            // The deletion, the last step, reads no binding: the match below goes straight to it.
            Err(_) if step == Step::Delete => Bindings::default(),
            Err(unread) => {
                self.status = Some(unread.appended_to(said(
                    "The new name was saved, but shortcut bindings could not be read; the old template was kept: ",
                )));
                return;
            }
        };
        let renamed_keys = shortcuts::keys_renamed(&bindings.keys, &old, &new);
        let (key, command, step) = match step {
            Step::Mark if bindings.is_marked(&old) => (
                OpKey::preferences(),
                shortcuts::mark_command(Some(&new)).ok(),
                Step::Mark,
            ),
            Step::Mark | Step::Keys if renamed_keys != bindings.keys => (
                OpKey::preferences(),
                shortcuts::keys_command(&renamed_keys).ok(),
                Step::Keys,
            ),
            Step::Put | Step::Mark | Step::Keys | Step::Delete => {
                (OpKey::template(&old), delete_command(&old), Step::Delete)
            }
        };
        let started = command.is_some_and(|command| {
            self.start(
                &key,
                command,
                Writing::Rename {
                    old: old.clone(),
                    new: new.clone(),
                    step,
                },
                cx,
            )
        });
        // Only a retired store refuses to start a step here (the editor's busy state keeps every other write off these
        // keys), so this is the old relay's wording for a write cut short, kept for the delete as for the moves.
        if !started {
            self.status = Some(said(
                "The new name was saved, but a shortcut write was interrupted; the old template was kept",
            ));
        }
    }

    // ---- Drawing ----------------------------------------------------------------------------------------------------

    /// The editor pane (`section.templates-editor`): heading, name, quick-launch line, switch, the fields it sets, the
    /// add menu, the hint and the footer with Save. `busy` is whether the dialog has any write in flight, this editor's
    /// or a ⚡ or number-key write.
    pub(super) fn render(
        &mut self,
        busy: bool,
        bindings: &Bindings,
        catalog: Option<&[LaunchCatalogModel]>,
        weak: &WeakEntity<TemplatesDialog>,
        cx: &mut Context<TemplatesDialog>,
    ) -> AnyElement {
        let palette = theme::palette(cx);
        let hosts: Vec<HostView> = session::current_store(cx)
            .map(|store| {
                store
                    .read(cx)
                    .collections()
                    .ordered::<HostView>()
                    .into_iter()
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let recipes =
            session::current_store(cx).map(|store| Recipes::of(store.read(cx).collections()));
        let flags = self.typed_text(Text::Flags, cx);
        let rows = rows::Rows {
            editor: self,
            weak,
            palette,
            hosts: &hosts,
            // A catalog not yet pushed, or unreadable, lists nothing, as in the launcher: a stored recipe then shows
            // as retained until the catalog arrives.
            recipes: match &recipes {
                Some(Recipes::Listed(recipes)) => recipes,
                Some(Recipes::Loading | Recipes::Unreadable(_)) | None => &[],
            },
            flags: &flags,
            catalog,
        };
        let fields = &self.draft.fields;

        let mut pane = div()
            .id("templates-editor")
            .role(Role::Region)
            .aria_label("Template editor")
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(EDITOR_GAP))
            .child(self.heading(palette))
            .child(rows.name_field());
        if self.read_only(Text::TemplateName) {
            pane = pane
                .child(rows::hint(
                    "templates-name-escaped",
                    "This name contains invisible formatting, shown as escapes. It stays unchanged until replaced.",
                    palette,
                ))
                .child(
                    div().flex().child(
                        Button::new(
                            "templates-replace-name",
                            "Replace name",
                            HoverHelp::tooltip("Clear the name so a new one can be typed"),
                            Role::Button,
                        )
                        .tier(Tier::Neutral)
                        .busy(busy)
                        .on_click(on_editor(weak, |editor, window, cx| {
                            editor.replace_name(window, cx)
                        })),
                    ),
                );
        }
        pane = pane
            .child(self.quick_state(bindings, palette))
            .child(rows.switch());
        if self.previous.is_some() && self.baseline.fields.kind.is_none() && fields.kind.is_some() {
            pane = pane.child(rows::hint(
                "templates-inferred",
                "This stored template does not switch the launcher yet. Saving will store the switch inferred from its \
                 fields.",
                palette,
            ));
        }
        pane = pane.child(
            div()
                .text_size(metrics::TEXT_MD)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::color(palette.fg_1))
                .child("Sets"),
        );
        for field in Field::ALL.into_iter().filter(|field| field.is_set(fields)) {
            pane = pane.child(rows.field(field));
        }
        pane.child(self.add_area(busy, weak, palette))
            .child(rows::hint(
                "templates-fields-hint",
                "Fields not shown are left as is. Another template can supply choices this one leaves out.",
                palette,
            ))
            .child(self.footer(busy, weak, palette, cx))
            .into_any_element()
    }

    /// The heading row: "Edit template" or "New template", and "Unsaved changes" while the draft differs from its
    /// baseline.
    fn heading(&self, palette: &Palette) -> impl IntoElement {
        let title = if self.previous.is_some() {
            "Edit template"
        } else {
            "New template"
        };
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(HEADING_TEXT))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_0))
                    .child(title),
            )
            .children(self.dirty().then(|| {
                div()
                    .debug_selector(|| "templates-unsaved".into())
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.warn))
                    .child("Unsaved changes")
            }))
    }

    /// The quick-launch line (`.templates-quick-state`): whether the saved template holds ⚡, and otherwise whether the
    /// draft could. The mark is judged on the stored name, eligibility on the draft, as the old editor judged them.
    fn quick_state(&self, bindings: &Bindings, palette: &Palette) -> impl IntoElement {
        let text = match self.previous.as_deref() {
            Some(previous) if bindings.is_marked(previous) => format!(
                "⚡ Quick launch: {} starts a session from this saved template.",
                super::QUICK_LAUNCH_SHORTCUT_NAME
            ),
            _ if quick_launch_eligible(&self.draft.fields) => {
                "Not the quick launch. The ⚡ in the list makes it one.".to_owned()
            }
            _ => "Can't be the quick launch: an agent launch needs an agent; a command launch needs a nonblank command."
                .to_owned(),
        };
        div()
            .debug_selector(|| "templates-quick-state".into())
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(palette.fg_2))
            .child(text)
    }

    /// "+ Add field" and, while it is open, its menu (`.templates-add-menu`): every field the template's switch and
    /// agent type offer and it does not set yet, two to a line, in the editor's order.
    fn add_area(
        &self,
        busy: bool,
        weak: &WeakEntity<TemplatesDialog>,
        palette: &Palette,
    ) -> impl IntoElement {
        let toggle = Button::new(
            "templates-add",
            "+ Add field",
            HoverHelp::tooltip(
                "Add a field this template will set; fields not shown are left as is",
            ),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .focus(&self.add_focus)
        .expanded(self.add_open)
        .busy(busy)
        .on_click(on_editor(weak, |editor, _, cx| {
            if editor.writing.is_none() {
                editor.add_open = !editor.add_open;
                cx.notify();
            }
        }));
        let area = div().flex().flex_col().child(div().flex().child(toggle));
        if !self.add_open {
            return area;
        }
        let offered: Vec<Field> = Field::ALL
            .into_iter()
            .filter(|field| !field.is_set(&self.draft.fields) && field.offered(&self.draft.fields))
            .collect();
        let items: Vec<AnyElement> = offered
            .iter()
            .map(|&field| {
                let noun = field.label().to_ascii_lowercase();
                Button::new(
                    format!("templates-add-{}", field.key()),
                    field.label(),
                    HoverHelp::tooltip(format!("Add {noun} to the template")),
                    Role::Button,
                )
                .tier(Tier::TemplateRow)
                .busy(busy)
                .on_click(on_editor(weak, move |editor, window, cx| {
                    editor.add(field, window, cx)
                }))
                .into_any_element()
            })
            .collect();
        let mut lines = Vec::new();
        let mut items = items.into_iter();
        while let Some(first) = items.next() {
            let second = items.next().unwrap_or_else(|| div().into_any_element());
            lines.push(
                div()
                    .flex()
                    .gap(px(ADD_MENU_GAP))
                    .child(div().flex_1().min_w_0().child(first))
                    .child(div().flex_1().min_w_0().child(second)),
            );
        }
        area.child(
            div()
                .id("templates-add-menu")
                .role(Role::Group)
                .aria_label("Add field")
                .mt(px(ADD_MENU_PAD))
                .p(px(ADD_MENU_PAD))
                .flex()
                .flex_col()
                .gap(px(ADD_MENU_GAP))
                .rounded(metrics::RADIUS_SM)
                .border_1()
                .border_color(theme::color(palette.border_dialog))
                .bg(theme::color(palette.bg_1))
                .children(lines),
        )
    }

    /// The editor's own footer (`.templates-editor-footer`, the shared dialog footer's look inside the editor):
    /// Duplicate and Delete for a stored template, then Save, at its right. Save is busy, and so greyed and inert while
    /// keeping focus, when there is nothing unsaved and while any write of the dialog's is in flight (`busy`).
    fn footer(
        &mut self,
        busy: bool,
        weak: &WeakEntity<TemplatesDialog>,
        palette: &Palette,
        cx: &App,
    ) -> impl IntoElement {
        let (pad_y, pad_x) = metrics::DIALOG_FOOTER_PAD;
        let band = div()
            .mt_auto()
            .py(pad_y)
            .px(pad_x)
            .flex()
            .flex_wrap()
            .justify_end()
            .gap(px(8.))
            .bg(theme::color(palette.bg_1))
            .border_t_1()
            .border_color(theme::color(palette.border_dim));
        let save = Button::new(
            "templates-save",
            "Save",
            HoverHelp::tooltip("Save the template's name, switch and shown fields").below(),
            Role::Button,
        )
        .tier(Tier::Primary)
        .focus(&self.save_focus)
        // `busy` is any write of the dialog's (this editor's, or a ⚡ or number-key write): they go one at a time.
        .busy(busy || !self.dirty())
        // A busy button cannot say so to assistive technology, so the name does while there is nothing to save.
        .accessible_name(if self.dirty() {
            "Save"
        } else {
            "Save, no unsaved changes"
        })
        .on_click({
            let weak = weak.clone();
            move |_, window, cx| {
                let _ = weak.update(cx, |dialog, cx| dialog.save_editor(window, cx));
            }
        });
        // Duplicate, for a stored template only, before Save, as the old footer ordered them.
        let duplicate = self.previous.is_some().then(|| {
            let weak = weak.clone();
            Button::new(
                "templates-duplicate",
                "Duplicate",
                HoverHelp::tooltip("Open a new unsaved copy with a free name").below(),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .busy(busy)
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |dialog, cx| dialog.duplicate(window, cx));
            })
        });
        // Delete, for a stored template only, between Duplicate and Save as in the old footer. The handler holds the
        // kind this frame's label says ([`DeleteKind`]).
        let delete = self.previous.is_some().then(|| {
            let weak = weak.clone();
            let kind = self.delete_kind(cx);
            let (label, help) = match kind {
                DeleteKind::Restorable => (
                    "Delete",
                    "Delete this template now; undo is available here for about ten seconds",
                ),
                DeleteKind::Permanent => (
                    "Delete permanently",
                    "Delete this template now; it fails current save rules, so undo is not available",
                ),
            };
            Button::new("templates-delete", label, HoverHelp::tooltip(help).below(), Role::Button)
                .tier(Tier::Danger)
                .busy(busy)
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |dialog, cx| dialog.delete_editor(kind, window, cx));
                })
        });
        band.children(duplicate).children(delete).child(save)
    }
}

/// Mirror `text`, typed into the input `which`, into `draft`, as the old editor's inputs did: an empty model or resume
/// command is the reset, an empty wrapper prefix is "no prefix", a wrapper prefix that does not split stays as typed
/// beside the last one that did, Custom words that do not parse are none, and the destination keeps its kind.
fn apply_text(draft: &mut Draft, which: Text, text: String) {
    let fields = &mut draft.fields;
    match which {
        Text::TemplateName => draft.name = text,
        Text::Flags => fields.permission_flags = farhelm_proto::split_permission_flags(&text).ok(),
        Text::Field(Field::Command) => fields.command = Some(text),
        Text::Field(Field::Model) => fields.model = Some((!text.is_empty()).then_some(text)),
        Text::Field(Field::Resume) => {
            fields.resume_command = Some((!text.is_empty()).then_some(text));
        }
        Text::Field(Field::Prompt) => fields.prompt = Some(text),
        Text::Field(Field::Name) => fields.name = Some(text),
        Text::Field(Field::Destination) => {
            fields.destination = Some(match fields.destination {
                Some(TemplateDestination::Github(_)) => TemplateDestination::Github(text),
                Some(TemplateDestination::Folder(_)) | None => TemplateDestination::Folder(text),
            });
        }
        Text::Field(Field::Wrapper) => {
            if text.is_empty() {
                fields.wrapper_prefix = Some(None);
                draft.wrapper_input = None;
            } else {
                match farhelm_proto::split_wrapper_prefix(&text) {
                    Ok(words) => {
                        fields.wrapper_prefix = Some(Some(words));
                        draft.wrapper_input = None;
                    }
                    Err(_) => draft.wrapper_input = Some(text),
                }
            }
        }
        Text::Field(
            Field::Yolo
            | Field::Agent
            | Field::Effort
            | Field::Permissions
            | Field::Container
            | Field::Host,
        ) => {}
    }
}

/// What sets one text input apart: its element id (which tests and the test bridge find it by), its label (its
/// accessible name, and the words above it), its hover help, its placeholder and whether it shows shell text or a path
/// in the monospace face, as the old `.templates-code` inputs did. All of them the old editor's.
pub(super) struct TextSpec {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) help: &'static str,
    pub(super) placeholder: Option<&'static str>,
    pub(super) monospace: bool,
}

/// The [`TextSpec`] of `which` while the draft holds `fields`: the destination's follows its kind (a repository or a
/// folder path).
pub(super) fn spec(which: Text, fields: &TemplateFields) -> TextSpec {
    let (id, label, help, placeholder, monospace) = match which {
        Text::TemplateName => (
            "templates-name",
            "Name",
            "Template name: use tl:name to apply it in New session",
            Some("my-claude"),
            false,
        ),
        Text::Flags => (
            "templates-flags",
            "Permission flags",
            "Custom permission flags: your flags replace Farhelm's permission flags; Farhelm does not check them and \
             treats this as YOLO",
            None,
            true,
        ),
        Text::Field(Field::Command) => (
            "templates-command",
            "Command",
            "Command: the command this template puts in the launcher",
            Some("agent-tool {farhelm_args}"),
            true,
        ),
        Text::Field(Field::Model) => (
            "templates-model",
            "Model",
            "Model: type any model; suggestions follow the agent type this template sets",
            Some("Default, or type any model"),
            false,
        ),
        Text::Field(Field::Wrapper) => (
            "templates-wrapper",
            "Wrapper prefix",
            "Wrapper prefix: Bash text before the launch command. Quotes and expansions keep their shell meaning. \
             There are no placeholders.",
            Some("with-endpoint --profile work --"),
            true,
        ),
        Text::Field(Field::Resume) => (
            "templates-resume",
            "Resume command",
            "Resume command: an empty input resets it to the default",
            Some("Default, or agent-tool --resume {conversation} {farhelm_args}"),
            true,
        ),
        Text::Field(Field::Prompt) => (
            "templates-prompt",
            "Initial prompt",
            "Initial prompt: the first message on each launch from this template",
            None,
            false,
        ),
        Text::Field(Field::Name) => (
            "templates-session-name",
            "Session name",
            "Session name: the name this template puts in the launcher",
            Some("review"),
            false,
        ),
        Text::Field(Field::Destination)
            if matches!(fields.destination, Some(TemplateDestination::Github(_))) =>
        {
            (
                "templates-destination",
                "Repository",
                "Repository: each session gets a managed checkout",
                Some("owner/name"),
                false,
            )
        }
        Text::Field(Field::Destination) => (
            "templates-destination",
            "Folder path",
            "Folder path: the working folder on the selected host",
            Some("/path/to/project"),
            true,
        ),
        // Choices, which have no input; never asked for.
        Text::Field(
            Field::Yolo
            | Field::Agent
            | Field::Effort
            | Field::Permissions
            | Field::Container
            | Field::Host,
        ) => ("templates-choice", "", "", None, false),
    };
    TextSpec {
        id,
        label,
        help,
        placeholder,
        monospace,
    }
}

/// A text input for `which`, subscribed so the user's edits reach the editor, with its [`TextSpec`]'s placeholder.
/// The prompt is the growing box, four rows at the start (SPEC.md: "a multiline box that grows with its text up to a
/// cap and then scrolls"); every other input is one line.
fn make_input(
    which: Text,
    fields: &TemplateFields,
    window: &mut Window,
    cx: &mut Context<TemplatesDialog>,
) -> Input {
    let state = if which == Text::Field(Field::Prompt) {
        TextInputState::new_growing(PROMPT_ROWS, window, cx)
    } else {
        TextInputState::new(window, cx)
    };
    if let Some(placeholder) = spec(which, fields).placeholder {
        state.set_placeholder(placeholder, window, cx);
    }
    let events = state.subscribe(
        window,
        cx,
        move |dialog: &mut TemplatesDialog, event, window, cx| {
            if event == TextInputEvent::Changed
                && let Some(editor) = dialog.editor.as_mut()
            {
                editor.typed(which, window, cx);
            }
        },
    );
    Input {
        state,
        _events: events,
    }
}

/// `DELETE /api/templates/{name}`: Delete, and a rename's last step, removing the old name. Work on the name's key, of
/// the save kind, since the store treats every editor write alike (busy on its own key, never superseded). `None` for a
/// name that cannot be a path segment, which a stored name never is.
fn delete_command(name: &str) -> Option<Command> {
    let path = ApiPath::api(&["templates", name]).ok()?;
    Some(Command::new(
        Work::Save,
        Method::Delete,
        path,
        None,
        "DELETE /api/templates/[template]",
    ))
}

/// The names of the templates the helm pushed, or `None` while it reports the collection unavailable or before its
/// first snapshot: a list that proves nothing about which names are free (see the module docs).
pub(super) fn stored_names(cx: &App) -> Option<Vec<String>> {
    let store = session::current_store(cx)?;
    let collections = store.read(cx).collections();
    match &collections.meta(Collection::Templates)?.availability {
        Availability::Available => Some(
            collections
                .ordered::<LaunchTemplate>()
                .into_iter()
                .map(|template| template.name.clone())
                .collect(),
        ),
        Availability::Unavailable { .. } => None,
    }
}

/// `PUT /api/templates/{name}` with the template's fields, whole: the editor's save (and a new template's, with no
/// previous name). `None` for a template whose name cannot be a path segment or whose fields do not serialize, which
/// the shape check refuses first.
pub(super) fn save_command(template: &LaunchTemplate) -> Option<Command> {
    let path = ApiPath::api(&["templates", &template.name]).ok()?;
    let body = serde_json::to_value(&template.fields).ok()?;
    Some(Command::new(
        Work::Save,
        Method::Put,
        path,
        Some(body),
        "PUT /api/templates/[template]",
    ))
}

/// A click handler that runs `act` on the dialog's open editor, if it still has one.
pub(super) fn on_editor(
    weak: &WeakEntity<TemplatesDialog>,
    act: impl Fn(&mut Editor, &mut Window, &mut Context<TemplatesDialog>) + 'static,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
    let weak = weak.clone();
    move |_, window, cx| with_editor(&weak, window, cx, &act)
}

/// Run `act` on the dialog's open editor, if the dialog is still open and has one, and redraw.
pub(super) fn with_editor(
    weak: &WeakEntity<TemplatesDialog>,
    window: &mut Window,
    cx: &mut App,
    act: impl FnOnce(&mut Editor, &mut Window, &mut Context<TemplatesDialog>),
) {
    let _ = weak.update(cx, |dialog, cx| {
        if let Some(editor) = dialog.editor.as_mut() {
            act(editor, window, cx);
        }
        cx.notify();
    });
}

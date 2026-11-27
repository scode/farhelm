//! Settings sequences explicit saves while each form owns its draft, command and outcome text, and holds a close until
//! the work out settles.
//!
//! SPEC.md, "Settings": the helm is a presumed low-latency, ordinarily reliable remote, so nothing here lets someone
//! move on before a helm round trip settles. Here's the TLDR of the mechanics:
//!
//! - **One save loop** ([`SaveLoop`]): a page Save or the close question's Save sends the drafts it approved one at a
//!   time, each after the one before it is saved, and stops at the first refusal with that page shown.
//! - **A close waits** ([`PendingClose`]): Close or Escape while anything that holds a close is out
//!   ([`SettingsDialog::close_busy`]) asks nothing yet. Each Settings-owned operation's own completion resumes it,
//!   through a one-shot continuation on the form ([`SaveReceipt`]) or from the dialog's own switch completions, after
//!   the form's follow-up updates; only a preference save no Settings form owns is left to the operation store's
//!   notification. The resumed close shows a refused page, or the Credentials overview after a missed delivery, or
//!   asks its question.

use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;

use farhelm_proto::{credentials::CredentialKind, custom_agents::CustomAgent};
use gpui::{
    App, Context, Div, Entity, EntityId, FocusHandle, ParentElement as _, Styled as _, Window, div,
    px,
};

use super::{
    CheckoutField, Section, SettingsDialog, credentials, custom_agents,
    file_types::FileTypes,
    ntfy::NtfyAlerts,
    recipes,
    tensorlake_key::{Place, TensorlakeKey},
};
use crate::components::{Button, HoverHelp, Tier};
use crate::focus::{self, Mark};
use crate::layers::{self, Dismissal};
use crate::operations::{OpKey, Outcome};
use crate::prompt::{self, Answer, Question};
use crate::session;
use crate::theme;

/// An unsaved marker uses inverse ink inside the selected row and accent ink anywhere else.
pub(super) fn dot(selected: bool, palette: &theme::Palette) -> Div {
    div()
        .flex_none()
        .size(px(6.))
        .rounded_full()
        .bg(theme::color(if selected {
            palette.on_fill
        } else {
            palette.accent
        }))
}

/// Place the marker after a field's title without widening or moving the field underneath it.
pub(super) fn caption(text: &'static str, edited: Option<&theme::Palette>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(text)
        .children(edited.map(|palette| dot(false, palette)))
}

/// Saving counts as unsaved until the operation settles, even if the draft matches the last loaded value.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum EditState {
    Clean,
    Dirty,
    Saving,
}

impl EditState {
    pub(super) fn of(saving: bool, dirty: bool) -> Self {
        if saving {
            Self::Saving
        } else if dirty {
            Self::Dirty
        } else {
            Self::Clean
        }
    }
}

/// Stage the visible result of a rejected close after the earlier key write settles. The interaction tests drive the
/// close and receipt ordering. This fixture uses the real masked input and conflict reveal to pin the resulting view.
#[cfg(test)]
pub(crate) fn stage_settled_key_conflict(window: &mut Window, cx: &mut App) -> Option<()> {
    let dialog = layers::last_opened::<SettingsDialog>(cx)?;
    let key = dialog.read(cx).credentials.as_ref()?.read(cx).key();
    if key.read(cx).typed_key().is_none() {
        key.read(cx)
            .replacement_input_for_golden(cx)?
            .edit_for_golden("fixture-replacement-key", window, cx);
        return None;
    }
    dialog.update(cx, |dialog, cx| dialog.reveal_key_conflict(window, cx));
    Some(())
}

/// Seed the refusal once the Notifications draft exists, then let Settings reveal it at actual layout.
#[cfg(test)]
pub(crate) fn stage_ntfy_refusal(window: &mut Window, cx: &mut App) -> Option<()> {
    let dialog = layers::last_opened::<SettingsDialog>(cx)?;
    let alerts = dialog.read(cx).ntfy.clone()?;
    let form = alerts.read(cx);
    if form.search_focus(super::ntfy::Switch::Enabled).is_none() || form.save_refused() {
        return None;
    }
    if form.edit_state() == EditState::Clean {
        form.topic_input().edit_for_golden("lab/alerts", window, cx);
    } else {
        alerts.update(cx, NtfyAlerts::refuse_topic_for_golden);
        dialog.update(cx, |dialog, cx| {
            dialog.scroll_to_close_refusal(cx);
            cx.notify();
        });
    }
    Some(())
}

type OnSaveSettled = Box<dyn FnOnce(bool, &mut App)>;

/// The dialog's two one-shot hooks on a form's work. The save continuation lets the save loop join an explicit Save
/// that a form already sent, or wait on one it just started. The close continuation lets a close that is waiting
/// ([`PendingClose`]) resume when the form's save, Test, Clear or Delete settles, and hears whether it was refused.
/// Both run after the entity update and the original completion's deferred field updates, so they may read the form
/// and go on without reentering the form's update.
#[derive(Default)]
pub(super) struct SaveReceipt {
    then: Option<OnSaveSettled>,
    close: Option<OnSaveSettled>,
}

impl SaveReceipt {
    /// Only the latest loop owns the continuation for a save that is still in flight.
    pub(super) fn wait(&mut self, then: impl FnOnce(bool, &mut App) + 'static) {
        self.then = Some(Box::new(then));
    }

    /// Arm the close continuation, which hears `true` when the work it waits on was refused.
    pub(super) fn wait_close(&mut self, then: impl FnOnce(bool, &mut App) + 'static) {
        self.close = Some(Box::new(then));
    }

    /// Whether a waiting close already armed this form.
    pub(super) fn close_armed(&self) -> bool {
        self.close.is_some()
    }

    /// A queued action advances only after the helm confirms a save. Refusals and unexpected deferred outcomes leave
    /// the form's own error or status in charge of what the user sees next.
    pub(super) fn settle(&mut self, outcome: &Outcome, cx: &mut App) {
        self.finish(matches!(outcome, Outcome::Done(_)), cx);
    }

    /// A save settled, `saved` or not. Editors that also run outside Settings retain their own definition of an
    /// acknowledged save.
    pub(super) fn finish(&mut self, saved: bool, cx: &mut App) {
        if let Some(then) = self.then.take() {
            cx.defer(move |cx| then(saved, cx));
        }
        self.settled_other(!saved, cx);
    }

    /// Work other than a save (a Test, a Clear, a Delete, or a switch's own save) settled, `refused` or not: only a
    /// waiting close hears of it. Call it last in the completion, after the form's own deferred updates are queued.
    pub(super) fn settled_other(&mut self, refused: bool, cx: &mut App) {
        if let Some(then) = self.close.take() {
            cx.defer(move |cx| then(refused, cx));
        }
    }
}

/// Save targets name the actual view holding an edit. Keeping that identity lets a queued save skip an editor
/// the user already left and avoids applying the old action to a replacement editor.
#[derive(Clone)]
enum Edit {
    FileTypes(Entity<FileTypes>),
    Alerts(Entity<NtfyAlerts>),
    Checkout(Entity<CheckoutField>),
    Key(Place, Entity<TensorlakeKey>),
    Recipe(Entity<recipes::Editor>),
    Credential(Entity<credentials::Editor>),
    CustomAgent(Section, Entity<custom_agents::Editor>),
}

/// What one Save or Discard authorized. Keep identities independent of values so closing a question does not keep
/// a detached editor alive.
#[derive(Clone)]
pub(super) struct ApprovedEdit {
    id: EntityId,
    version: EditVersion,
}

#[derive(Clone, PartialEq)]
enum EditVersion {
    Input(u64),
    Alerts((u64, bool)),
    Checkout((u64, u64)),
    Key((FocusHandle, u64)),
    Recipe(Box<recipes::draft::Draft>),
    Credential((FocusHandle, u64, bool, String, CredentialKind)),
    CustomAgent((CustomAgent, bool, bool)),
}

impl ApprovedEdit {
    /// Compare both identity and the inputs that determine the write.
    fn permits(&self, edit: &Edit, cx: &App) -> bool {
        self.id == edit.id() && self.version == edit.version(cx)
    }
}

impl Edit {
    fn approval(&self, cx: &App) -> ApprovedEdit {
        ApprovedEdit {
            id: self.id(),
            version: self.version(cx),
        }
    }

    /// Text fields that own secrets contribute only their revision and source identity.
    fn version(&self, cx: &App) -> EditVersion {
        match self {
            Self::FileTypes(view) => EditVersion::Input(view.read(cx).edit_revision()),
            Self::Alerts(view) => EditVersion::Alerts(view.read(cx).edit_revision()),
            Self::Checkout(view) => EditVersion::Checkout(view.read(cx).edit_revision()),
            Self::Key(_, view) => EditVersion::Key(view.read(cx).edit_revision(cx)),
            Self::Recipe(view) => EditVersion::Recipe(Box::new(view.read(cx).draft(cx))),
            Self::Credential(view) => EditVersion::Credential(view.read(cx).edit_revision(cx)),
            Self::CustomAgent(_, view) => EditVersion::CustomAgent(view.read(cx).edit_revision(cx)),
        }
    }

    fn id(&self) -> EntityId {
        match self {
            Self::FileTypes(view) => view.entity_id(),
            Self::Alerts(view) => view.entity_id(),
            Self::Checkout(view) => view.entity_id(),
            Self::Key(_, view) => view.entity_id(),
            Self::Recipe(view) => view.entity_id(),
            Self::Credential(view) => view.entity_id(),
            Self::CustomAgent(_, view) => view.entity_id(),
        }
    }

    /// The page to reveal if the form refuses the save, even when the form was hidden when it was sent.
    fn section(&self) -> Section {
        match self {
            Self::FileTypes(_) => Section::Terminal,
            Self::Alerts(_) => Section::Notifications,
            Self::Checkout(_) => Section::Checkouts,
            Self::Key(Place::Tensorlake, _) => Section::Tensorlake,
            Self::Key(Place::Credentials, _) | Self::Credential(_) => Section::Credentials,
            Self::Recipe(_) => Section::Recipes,
            Self::CustomAgent(section, _) => section.clone(),
        }
    }

    /// The question names the form when the page contains several controls that save separately.
    fn name(&self) -> &'static str {
        match self {
            Self::FileTypes(_) => "File types",
            Self::Alerts(_) => "ntfy alerts",
            Self::Checkout(_) => "Checkout settings",
            Self::Key(Place::Tensorlake, _) => "Tensorlake API key",
            Self::Key(Place::Credentials, _) => "Tensorlake API key in Credentials",
            Self::Recipe(_) => "Recipe",
            Self::Credential(_) => "Credential",
            Self::CustomAgent(_, _) => "Custom agent",
        }
    }

    fn state(&self, cx: &App) -> EditState {
        match self {
            Self::FileTypes(view) => view.read(cx).edit_state(),
            Self::Alerts(view) => view.read(cx).edit_state(),
            Self::Checkout(view) => view.read(cx).edit_state(),
            Self::Key(_, view) => view.read(cx).edit_state(),
            Self::Recipe(view) => view.read(cx).edit_state(cx),
            Self::Credential(view) => view.read(cx).edit_state(cx),
            Self::CustomAgent(_, view) => view.read(cx).edit_state(cx),
        }
    }

    /// A test or clear can lock the form without becoming a pending save in the close question.
    fn holds_save(&self, cx: &App) -> bool {
        match self {
            Self::Alerts(view) => view.read(cx).holds_save(),
            Self::Key(_, view) => view.read(cx).holds_save(),
            _ => false,
        }
    }

    /// Whether this form's work holds a close ([`SettingsDialog::close_busy`]): a save out, a Test or Clear, or an
    /// editor's Delete. A recipe's Delete carries on without the dialog, so it does not.
    fn holds_close(&self, cx: &App) -> bool {
        self.state(cx) == EditState::Saving
            || self.holds_save(cx)
            || match self {
                Self::Credential(view) => view.read(cx).deleting(),
                Self::CustomAgent(_, view) => view.read(cx).deleting(),
                _ => false,
            }
    }

    fn close_armed(&self, cx: &App) -> bool {
        match self {
            Self::FileTypes(view) => view.read(cx).save_receipt.close_armed(),
            Self::Alerts(view) => view.read(cx).save_receipt.close_armed(),
            Self::Checkout(view) => view.read(cx).save_receipt.close_armed(),
            Self::Key(_, view) => view.read(cx).save_receipt.close_armed(),
            Self::Recipe(view) => view.read(cx).save_receipt.close_armed(),
            Self::Credential(view) => view.read(cx).save_receipt.close_armed(),
            Self::CustomAgent(_, view) => view.read(cx).save_receipt.close_armed(),
        }
    }

    /// Arm this form's close continuation ([`SaveReceipt::wait_close`]).
    fn wait_close(&self, then: impl FnOnce(bool, &mut App) + 'static, cx: &mut App) {
        match self {
            Self::FileTypes(view) => view.update(cx, |form, _| form.save_receipt.wait_close(then)),
            Self::Alerts(view) => view.update(cx, |form, _| form.save_receipt.wait_close(then)),
            Self::Checkout(view) => view.update(cx, |form, _| form.save_receipt.wait_close(then)),
            Self::Key(_, view) => view.update(cx, |form, _| form.save_receipt.wait_close(then)),
            Self::Recipe(view) => view.update(cx, |form, _| form.save_receipt.wait_close(then)),
            Self::Credential(view) => view.update(cx, |form, _| form.save_receipt.wait_close(then)),
            Self::CustomAgent(_, view) => {
                view.update(cx, |form, _| form.save_receipt.wait_close(then))
            }
        }
    }

    fn is_form(&self) -> bool {
        !matches!(
            self,
            Self::Recipe(_) | Self::Credential(_) | Self::CustomAgent(..)
        )
    }

    /// Wait for an existing operation on this exact form. When starting a save, arm the receipt only after the
    /// form admits it. A synchronous validation failure already owns its inline error.
    fn save(
        &self,
        already_saving: bool,
        window: &mut Window,
        cx: &mut App,
        then: impl FnOnce(bool, &mut App) + 'static,
    ) -> bool {
        macro_rules! settle_with {
            ($view:expr, $save:expr) => {
                $view.update(cx, |form, cx| {
                    if already_saving || ($save)(form, cx) {
                        form.save_receipt.wait(then);
                        true
                    } else {
                        false
                    }
                })
            };
        }
        match self {
            Self::FileTypes(view) => {
                settle_with!(view, |form: &mut FileTypes, cx| form.save(window, cx))
            }
            Self::Alerts(view) => settle_with!(view, NtfyAlerts::save),
            Self::Checkout(view) => settle_with!(view, |form: &mut CheckoutField, cx| form
                .save_from_settings(window, cx)),
            Self::Key(_, view) => {
                settle_with!(view, |form: &mut TensorlakeKey, cx| form.save(window, cx))
            }
            Self::Recipe(view) => settle_with!(view, recipes::Editor::save),
            Self::Credential(view) => settle_with!(view, |form: &mut credentials::Editor, cx| form
                .save(window, cx)),
            Self::CustomAgent(_, view) => {
                settle_with!(view, |form: &mut custom_agents::Editor, cx| form
                    .save(window, cx))
            }
        }
    }

    /// The ordinary footer discards fields only. Editors keep their own Cancel and leave behavior.
    fn discard(&self, window: &mut Window, cx: &mut App) {
        match self {
            Self::FileTypes(view) => view.update(cx, |form, cx| form.discard(window, cx)),
            Self::Alerts(view) => view.update(cx, |form, cx| form.discard(window, cx)),
            Self::Checkout(view) => view.update(cx, CheckoutField::discard),
            Self::Key(_, view) => view.update(cx, TensorlakeKey::discard),
            Self::Recipe(_) | Self::Credential(_) | Self::CustomAgent(..) => {}
        }
    }
}

/// One Save press, scoped to a page or to closing Settings: the approved drafts still to send, one at a time. SPEC.md,
/// "Settings": the helm is a presumed low-latency remote, so the loop simply waits for each save before sending the
/// next, and a close waits for the whole loop. There is no follow-up batch and no watch over saves already out; a
/// close asked during the loop waits for it ([`PendingClose`]).
pub(super) struct SaveLoop {
    remaining: VecDeque<ApprovedEdit>,
    /// Whether the loop came from the close question's Save, and closes Settings once every listed draft is saved.
    close: bool,
    /// Whether a credential replacement this loop sent was saved but not delivered everywhere. Settings then stays
    /// open on the Credentials overview, where those hosts are listed, rather than closing over them.
    has_delivery_warnings: bool,
}

/// A Close or Escape that found Settings busy ([`SettingsDialog::close_busy`]) and waits for the work to settle before
/// asking its question or closing. Each Settings-owned operation's own completion resumes it
/// ([`SettingsDialog::close_settled`]), after that completion's deferred state updates, so the resumed close never
/// sees a form between its reply and its own follow-up (a saved key's field not yet emptied, say).
pub(super) struct PendingClose {
    /// The press that asked to close. Any later input means the person moved on, and the resumed close does nothing.
    mark: Mark,
    /// The Credentials overview's delivery-report count when the close was asked. A report that arrives while the close
    /// waits keeps Settings open on the overview; one already seen does not.
    delivery_generation: u64,
    /// The forms' receipts armed for this close and not yet fired ([`SaveReceipt::wait_close`]). While any is out, the
    /// operation store's own notification does not resume the close: that form's completion will.
    armed: Rc<Cell<usize>>,
    /// The first refusal met while waiting, which the resumed close shows instead of closing.
    refused: Option<CloseRefusal>,
}

impl PendingClose {
    /// Keep the first refusal met while waiting: it is the one the resumed close shows.
    pub(super) fn refuse(&mut self, refused: Option<CloseRefusal>) {
        if self.refused.is_none() {
            self.refused = refused;
        }
    }
}

/// What a refusal met by a waiting close shows: its page, and, when a form or editor was refused, that form, so the
/// form's own reveal runs (the Credentials overview's key form, say, in front of an open credential editor). The form
/// is named by its entity id, not held: a receipt holding its own form would keep it alive.
pub(super) struct CloseRefusal {
    section: Section,
    edit: Option<EntityId>,
}

impl CloseRefusal {
    /// A refusal of a switch on `section`, which has no form of its own.
    pub(super) fn page(section: Section) -> Self {
        CloseRefusal {
            section,
            edit: None,
        }
    }

    /// A refusal of `edit`'s save, Test, Clear or Delete.
    fn edit(edit: &Edit) -> Self {
        CloseRefusal {
            section: edit.section(),
            edit: Some(edit.id()),
        }
    }
}

/// One armed close receipt, counted in [`PendingClose::armed`] for as long as it lives: until its callback runs, or
/// until the form holding it goes without running it (a custom agent editor a push removed, say), so a lost receipt
/// cannot leave the count up and stop the operation store's notification from resuming the close.
struct Armed(Rc<Cell<usize>>);

impl Armed {
    fn new(count: &Rc<Cell<usize>>) -> Self {
        count.set(count.get() + 1);
        Armed(Rc::clone(count))
    }
}

impl Drop for Armed {
    fn drop(&mut self) {
        self.0.set(self.0.get().saturating_sub(1));
    }
}

impl SettingsDialog {
    /// Every loaded form and editor, clean or not, hidden or not. Include both Tensorlake key fields because they hold
    /// separate drafts, although a successful write changes the same stored account.
    fn loaded(&self, cx: &App) -> Vec<Edit> {
        let mut edits = Vec::new();
        edits.extend(self.file_types.iter().cloned().map(Edit::FileTypes));
        edits.extend(self.ntfy.iter().cloned().map(Edit::Alerts));
        if let Some(fields) = &self.checkout_fields {
            edits.push(Edit::Checkout(fields.folder.clone()));
            edits.push(Edit::Checkout(fields.post_clone.clone()));
        }
        if let Some(view) = &self.recipes
            && let Some(editor) = view.read(cx).editor()
        {
            edits.push(Edit::Recipe(editor));
        }
        if let Some(view) = &self.tensorlake_section {
            edits.push(Edit::Key(Place::Tensorlake, view.read(cx).key()));
        }
        if let Some(view) = &self.credentials {
            edits.push(Edit::Key(Place::Credentials, view.read(cx).key()));
            edits.extend(view.read(cx).editor().map(Edit::Credential));
        }
        edits.extend(self.custom_agents.iter().map(|(key, editor)| {
            let section = key.as_ref().map_or(Section::AddCustomAgent, |id| {
                Section::CustomAgent(id.clone())
            });
            Edit::CustomAgent(section, editor.clone())
        }));
        edits
    }

    /// The loaded drafts that are unsaved or saving: what the close question and the dirty markers name.
    fn edits(&self, cx: &App) -> Vec<Edit> {
        let mut edits = self.loaded(cx);
        edits.retain(|edit| edit.state(cx) != EditState::Clean);
        edits
    }

    pub(super) fn section_edited(&self, section: &Section, cx: &App) -> bool {
        self.edits(cx).iter().any(|edit| &edit.section() == section)
    }

    /// An editor owns its actions, but its dirty marker follows the same draft as the section and close question.
    pub(super) fn editor_note(&self, cx: &App) -> Option<&'static str> {
        self.edits(cx)
            .into_iter()
            .find(|edit| !edit.is_form() && edit.section() == self.shown)
            .map(|edit| edit.name())
    }

    /// Drafts can be discarded without sending a command. A save in flight still belongs to the form that sent it.
    pub(super) fn discard_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for edit in self.edits(cx) {
            if edit.is_form() && edit.section() == self.shown && edit.state(cx) == EditState::Dirty
            {
                edit.discard(window, cx);
            }
        }
        self.key_conflict = false;
        cx.notify();
    }

    pub(super) fn save_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let approved = self
            .edits(cx)
            .iter()
            .filter(|edit| edit.is_form() && edit.section() == self.shown)
            .map(|edit| edit.approval(cx))
            .collect();
        self.start_loop(approved, false, window, cx);
    }

    /// The current page has only one Save even when it holds several fields, as Checkouts does. Its identity
    /// survives the first field settling so keyboard focus stays on Save until the loop ends.
    pub(super) fn page_answers(&self, cx: &Context<Self>) -> Option<(&'static str, Vec<Button>)> {
        let edits: Vec<_> = self
            .edits(cx)
            .into_iter()
            .filter(|edit| edit.is_form() && edit.section() == self.shown)
            .collect();
        let name = edits.first()?.name();
        let saving = edits.iter().any(|edit| edit.state(cx) == EditState::Saving);
        let mut answers = Vec::new();
        if !saving {
            let dialog = cx.weak_entity();
            answers.push(
                Button::new(
                    "settings-footer-discard",
                    "Discard",
                    HoverHelp::tooltip("Restore the saved values on this page"),
                    gpui::Role::Button,
                )
                .tier(Tier::Ghost)
                .on_click(move |_, window, cx| {
                    let _ = dialog.update(cx, |dialog, cx| dialog.discard_page(window, cx));
                }),
            );
        }
        let dialog = cx.weak_entity();
        answers.push(
            Button::new(
                "settings-footer-save",
                if saving { "Saving…" } else { "Save" },
                HoverHelp::tooltip("Save the changes on this page"),
                gpui::Role::Button,
            )
            .accessible_name("Save")
            .tier(Tier::Primary)
            .busy(
                self.save_loop.is_some() || saving || edits.iter().any(|edit| edit.holds_save(cx)),
            )
            .on_click(move |_, window, cx| {
                let _ = dialog.update(cx, |dialog, cx| dialog.save_page(window, cx));
            }),
        );
        Some((name, answers))
    }

    // ---- The save loop ----------------------------------------------------------------------------------------------

    /// Send `approved` one at a time, each after the one before it is saved. A close Save first refuses two different
    /// replacement Tensorlake keys, since it cannot pick one of them for the one account.
    fn start_loop(
        &mut self,
        approved: Vec<ApprovedEdit>,
        close: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.save_loop.is_some() {
            return;
        }
        self.key_conflict = false;
        if close && self.keys_disagree(cx) {
            self.reveal_key_conflict(window, cx);
            return;
        }
        self.save_loop = Some(SaveLoop {
            remaining: approved.into(),
            close,
            has_delivery_warnings: false,
        });
        self.advance_loop(window, cx);
    }

    /// Send the next approved draft that is still loaded, or finish the loop. A draft changed since its approval is
    /// left in Settings for a fresh action (SPEC.md, "Settings"); a close Save shows its page. One that a Test or Clear
    /// now holds stops the loop and shows its page, since its save could not start.
    fn advance_loop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Some(save_loop) = &mut self.save_loop {
            let close = save_loop.close;
            let Some(approval) = save_loop.remaining.pop_front() else {
                self.finish_loop(window, cx);
                break;
            };
            let Some(next) = self
                .loaded(cx)
                .into_iter()
                .find(|edit| edit.id() == approval.id)
            else {
                continue;
            };
            if !approval.permits(&next, cx) {
                if close {
                    self.reveal_save_failure(next.section(), window, cx);
                }
                continue;
            }
            if next.holds_save(cx) {
                self.save_loop = None;
                self.reveal_edit_failure(&next, window, cx);
                // The save could not be sent, which a waiting close treats as a refusal of that form.
                self.close_settled(Some(CloseRefusal::edit(&next)), window, cx);
                break;
            }
            self.dispatch_edit(next, window, cx);
            break;
        }
        cx.notify();
    }

    /// Send `edit`'s save, or join the one it already has out (its editor's own Save, pressed while the loop ran).
    fn dispatch_edit(&mut self, edit: Edit, window: &mut Window, cx: &mut Context<Self>) {
        let dialog = cx.weak_entity();
        let handle = window.window_handle();
        let reports = self.delivery_generation(cx);
        let finished = edit.clone();
        let already_saving = edit.state(cx) == EditState::Saving;
        let started = edit.save(already_saving, window, cx, move |saved, cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = dialog.update(cx, |dialog, cx| {
                    dialog.edit_finished(&finished, saved, reports, window, cx)
                });
            });
        });
        if !started {
            self.edit_finished(&edit, false, reports, window, cx);
        }
    }

    /// `edit`'s save, sent by the loop when the overview's delivery reports stood at `reports`, settled: go on after a
    /// done save, and stop and show its page after a refused one.
    fn edit_finished(
        &mut self,
        edit: &Edit,
        saved: bool,
        reports: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let missed_hosts =
            matches!(edit, Edit::Credential(_)) && self.delivery_generation(cx) > reports;
        let Some(save_loop) = &mut self.save_loop else {
            return;
        };
        save_loop.has_delivery_warnings |= missed_hosts;
        if saved {
            self.advance_loop(window, cx);
            return;
        }
        let close = save_loop.close;
        self.save_loop = None;
        self.reveal_edit_failure(edit, window, cx);
        if close {
            self.scroll_to_close_refusal(cx);
        }
        // Also when the form refused the save itself (a draft its own check rejects), which no receipt reports.
        self.close_settled(Some(CloseRefusal::edit(edit)), window, cx);
    }

    /// Every approved draft was sent. A close Save closes Settings, unless a credential it saved missed hosts (shown on
    /// the Credentials overview instead), a draft that changed meanwhile still needs its own action (SPEC.md: "New or
    /// changed drafts stay in Settings for a fresh action"), or work started meanwhile (a switch's save, a Test) is
    /// still out, whose outcome a close would drop unseen; Settings then stays open, as for a newer draft.
    fn finish_loop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(save_loop) = self.save_loop.take() else {
            return;
        };
        if save_loop.close {
            if save_loop.has_delivery_warnings {
                self.show_delivery_reports(window, cx);
            } else if self.edits(cx).is_empty() && !self.close_busy(cx) {
                self.finish_close(window, cx);
            }
        }
        self.close_settled(None, window, cx);
    }

    /// Both forms write the same key. Refuse the close Save before any writes if the two drafts disagree.
    fn key_drafts<'a>(&self, cx: &'a App) -> Option<(&'a str, &'a str)> {
        let lake = self.tensorlake_section.as_ref()?.read(cx).key();
        let credentials = self.credentials.as_ref()?.read(cx).key();
        Some((
            lake.read(cx).typed_key()?,
            credentials.read(cx).typed_key()?,
        ))
    }

    fn keys_disagree(&self, cx: &App) -> bool {
        self.key_drafts(cx)
            .is_some_and(|(lake, credentials)| lake != credentials)
    }

    fn reveal_key_conflict(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.key_conflict = true;
        self.show_credential_key(cx);
        self.reveal_save_failure(Section::Credentials, window, cx);
    }

    fn show_credential_key(&mut self, cx: &mut Context<Self>) {
        if let Some(credentials) = &self.credentials {
            credentials.update(cx, credentials::Credentials::reveal_key);
        }
    }

    /// A failed key write can belong to the Credentials overview even while its separate editor is open, and a failed
    /// credential editor save brings that editor back in front of the overview, so the shown page always carries the
    /// error of the save that failed.
    fn reveal_edit_failure(&mut self, edit: &Edit, window: &mut Window, cx: &mut Context<Self>) {
        match edit {
            Edit::Key(Place::Credentials, _) => self.show_credential_key(cx),
            Edit::Credential(editor) => {
                if let Some(credentials) = &self.credentials {
                    credentials.update(cx, |credentials, cx| credentials.reveal_editor(editor, cx));
                }
            }
            _ => {}
        }
        self.reveal_save_failure(edit.section(), window, cx);
    }

    fn reveal_save_failure(
        &mut self,
        section: Section,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.search.clear(window, cx);
        if self.shown != section {
            self.show(section, window, cx);
        }
        cx.notify();
    }

    /// Notifications puts the ntfy refusal, and a failed Send test's line, below its form and the sound controls.
    /// Scroll after selecting the section and let the next layout measure the full page, including the new error.
    fn scroll_to_close_refusal(&mut self, cx: &App) {
        if self.shown == Section::Notifications
            && self.ntfy.as_ref().is_some_and(|alerts| {
                let alerts = alerts.read(cx);
                alerts.save_refused() || alerts.test_failed()
            })
        {
            self.scroll_to_ntfy_refusal = true;
        }
    }

    /// The Credentials overview's count of delivery reports in this opening ([`credentials::Credentials`]).
    fn delivery_generation(&self, cx: &App) -> u64 {
        self.credentials
            .as_ref()
            .map_or(0, |credentials| credentials.read(cx).delivery_generation())
    }

    /// Stay open on the top of the Credentials overview, where a save's missed hosts are listed.
    fn show_delivery_reports(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.clear(window, cx);
        if self.shown != Section::Credentials {
            self.show(Section::Credentials, window, cx);
        }
        self.page_scroll.set_offset(gpui::Point::default());
        cx.notify();
    }

    /// A pending close owns its completed saves. A newer interactive child needs its own answer before Settings closes.
    fn finish_close(&self, window: &mut Window, cx: &mut App) {
        if let Some(layer) = self.layer
            && !layers::has_interactive_child(layer, window, cx)
        {
            layers::close(layer, window, cx);
        }
    }

    // ---- Closing ----------------------------------------------------------------------------------------------------

    /// Whether a close must wait before it asks or closes: a draft saving, a Test or Clear holding a form, a credential
    /// or custom agent Delete out, the save loop running, or any save of the preference row (the switches outside
    /// forms, and writers outside Settings). Recipe Build, Rebuild and Delete, Containers' and Tensorlake's leftover
    /// work and Create SSH key do not hold it: they carry on, or tell their own outcome, without the dialog.
    pub(super) fn close_busy(&self, cx: &App) -> bool {
        self.save_loop.is_some()
            || self.loaded(cx).iter().any(|edit| edit.holds_close(cx))
            || session::current_operations(cx).is_some_and(|operations| {
                operations
                    .read(cx)
                    .in_flight(&OpKey::preferences())
                    .is_some()
            })
    }

    /// All three close routes ask the same question about the loaded drafts, including the ones hidden by search or
    /// another section. While work is out the close waits for it instead ([`PendingClose`]); a close Save already
    /// running closes Settings itself when it finishes.
    pub(super) fn ask_to_close(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Dismissal {
        if self
            .save_loop
            .as_ref()
            .is_some_and(|save_loop| save_loop.close)
        {
            return Dismissal::Stay;
        }
        if self.close_busy(cx) {
            self.wait_to_close(window, cx);
            return Dismissal::Stay;
        }
        self.close_waiting = None;
        let edits = self.edits(cx);
        if edits.is_empty() {
            return Dismissal::Close;
        }
        let mut names = Vec::new();
        for edit in &edits {
            if !names.contains(&edit.name()) {
                names.push(edit.name());
            }
        }
        if names.len() == 1 {
            self.reveal_save_failure(edits[0].section(), window, cx);
        }
        let approved: Vec<_> = edits.iter().map(|edit| edit.approval(cx)).collect();
        let discard = cx.weak_entity();
        let question = Question::new(
            "Save changes before closing?",
            Answer::new(
                "Save",
                HoverHelp::tooltip("Save all the listed changes, then close Settings"),
                Tier::Primary,
            ),
            approved,
        )
        .line(format!("Unsaved: {}", names.join(", ")))
        .cancel_answer(Answer::new(
            "Keep editing",
            HoverHelp::tooltip("Stay in Settings with this draft"),
            Tier::Neutral,
        ))
        .alternative(
            Answer::new(
                "Discard",
                HoverHelp::tooltip("Close Settings without saving the drafts"),
                Tier::Ghost,
            ),
            move |approved, window, cx| {
                let _ = discard.update(cx, |dialog, cx| {
                    // Closing drops every draft. A new or changed one needs its own answer.
                    if dialog.edits(cx).iter().all(|edit| {
                        edit.state(cx) == EditState::Dirty
                            && approved.iter().any(|approval| approval.permits(edit, cx))
                    }) {
                        dialog.finish_close(window, cx);
                    }
                });
            },
        );
        let save = cx.weak_entity();
        let mark = Mark::take(cx);
        prompt::ask(
            question,
            None,
            &mark,
            move |approved, window, cx| {
                let _ = save.update(cx, |dialog, cx| {
                    dialog.start_loop(approved, true, window, cx)
                });
            },
            window,
            cx,
        );
        Dismissal::Stay
    }

    /// Hold this close until the work out settles: remember the press (a repeated Close or Escape takes its place) and
    /// arm the receipts of the forms whose work is out.
    fn wait_to_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mark = Mark::take(cx);
        match &mut self.close_waiting {
            Some(pending) => pending.mark = mark,
            None => {
                self.close_waiting = Some(PendingClose {
                    mark,
                    delivery_generation: self.delivery_generation(cx),
                    armed: Rc::default(),
                    refused: None,
                });
            }
        }
        self.arm_close(window, cx);
    }

    /// Arm a close receipt on every form whose work holds the close and has none armed yet. The Tensorlake section's
    /// switch saves the preference row from a view of its own, so it gets one too.
    fn arm_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(armed) = self
            .close_waiting
            .as_ref()
            .map(|pending| Rc::clone(&pending.armed))
        else {
            return;
        };
        let handle = window.window_handle();
        let dialog = cx.weak_entity();
        let receipt = move |refusal: CloseRefusal| {
            let (guard, dialog) = (Armed::new(&armed), dialog.clone());
            move |refused: bool, cx: &mut App| {
                drop(guard);
                let _ = handle.update(cx, |_, window, cx| {
                    let _ = dialog.update(cx, |dialog, cx| {
                        dialog.close_settled(refused.then_some(refusal), window, cx)
                    });
                });
            }
        };
        for edit in self.loaded(cx) {
            if edit.holds_close(cx) && !edit.close_armed(cx) {
                edit.wait_close(receipt(CloseRefusal::edit(&edit)), cx);
            }
        }
        if let Some(view) = &self.tensorlake_section
            && view.read(cx).holds_close()
            && !view.read(cx).save_receipt.close_armed()
        {
            let then = receipt(CloseRefusal::page(Section::Tensorlake));
            view.update(cx, |view, _| view.save_receipt.wait_close(then));
        }
    }

    /// Something a waiting close may depend on settled, with the page of its refusal when it was refused. Once nothing
    /// holds the close, it goes on: it does nothing if the person moved on since asking, shows the first refused page,
    /// shows the Credentials overview if a credential save reported missed hosts meanwhile, and otherwise asks its
    /// question again (or closes, with nothing unsaved).
    pub(super) fn close_settled(
        &mut self,
        refused: Option<CloseRefusal>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pending) = &mut self.close_waiting else {
            return;
        };
        pending.refuse(refused);
        if pending.armed.get() > 0 {
            return;
        }
        if self.close_busy(cx) {
            self.arm_close(window, cx);
            return;
        }
        let Some(pending) = self.close_waiting.take() else {
            return;
        };
        if focus::has_moved_on(&pending.mark, cx) {
            return;
        }
        if let Some(refusal) = pending.refused {
            let refused = refusal
                .edit
                .and_then(|id| self.loaded(cx).into_iter().find(|edit| edit.id() == id));
            match refused {
                Some(edit) => self.reveal_edit_failure(&edit, window, cx),
                None => self.reveal_save_failure(refusal.section, window, cx),
            }
            self.scroll_to_close_refusal(cx);
        } else if self.delivery_generation(cx) > pending.delivery_generation {
            self.show_delivery_reports(window, cx);
        } else if self.ask_to_close(window, cx) == Dismissal::Close {
            self.finish_close(window, cx);
        }
    }

    /// The operation store changed. A close waiting on a preference save that no Settings form owns (a switch outside
    /// Settings, another writer) has no completion of its own to resume it, so this does; while any armed receipt is
    /// out it leaves the close to that form's completion, which runs after the form's own follow-up.
    pub(super) fn operations_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .close_waiting
            .as_ref()
            .is_some_and(|pending| pending.armed.get() == 0)
            && !self.close_busy(cx)
        {
            self.close_settled(None, window, cx);
        }
    }
}

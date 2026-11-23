//! Settings sequences explicit saves while each form owns its draft, command and outcome text.

use std::collections::VecDeque;

use gpui::{App, Context, Div, Entity, EntityId, ParentElement as _, Styled as _, Window, div, px};

use super::{
    CheckoutField, Section, SettingsDialog, credentials, custom_agents,
    file_types::FileTypes,
    ntfy::NtfyAlerts,
    recipes,
    tensorlake_key::{Place, TensorlakeKey},
};
use crate::components::{Button, HoverHelp, Tier};
use crate::focus::Mark;
use crate::layers::{self, Dismissal, LayerId};
use crate::operations::Outcome;
use crate::prompt::{self, Answer, Question};
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

type OnSaveSettled = Box<dyn FnOnce(bool, &mut App)>;

/// The dialog can join an explicit Save that a form already sent, or wait on one it just started. The continuation
/// runs after the entity update and the original completion's deferred field updates, so it may read the form and
/// advance the batch without reentering the form's update.
#[derive(Default)]
pub(super) struct SaveReceipt(Option<OnSaveSettled>);

impl SaveReceipt {
    /// Only the latest close or footer action owns the continuation for a save that is still in flight.
    pub(super) fn wait(&mut self, then: impl FnOnce(bool, &mut App) + 'static) {
        self.0 = Some(Box::new(then));
    }

    /// A queued action advances only after the helm confirms a save. Refusals and unexpected deferred outcomes leave
    /// the form's own error or status in charge of what the user sees next.
    pub(super) fn settle(&mut self, outcome: &Outcome, cx: &mut App) {
        self.finish(matches!(outcome, Outcome::Done(_)), cx);
    }

    /// Editors that also run outside Settings retain their own definition of an acknowledged save.
    pub(super) fn finish(&mut self, saved: bool, cx: &mut App) {
        if let Some(then) = self.0.take() {
            cx.defer(move |cx| then(saved, cx));
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

impl Edit {
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

/// One Save button press, scoped to a page or to closing Settings. Existing saves may already be out on different
/// keys. Observe all of them before dispatching new work, then wait after each new save so the shared preference key
/// is available to the next form.
pub(super) struct Batch {
    generation: u64,
    close: bool,
    check_keys: bool,
    /// The receipt lives in the view. Hold it even if the user leaves its credential editor during the write.
    active: Vec<Edit>,
    remaining: VecDeque<Edit>,
    /// After the first of two matching keys has gone, the second must still agree with the key that was sent.
    paired_key: Option<String>,
    after: AfterBatch,
}

/// A close asked during an authorized page Save does not take the save's receipt away.
enum AfterBatch {
    None,
    Question(LayerId),
    SaveAndClose,
}

impl SettingsDialog {
    /// Loaded views remain in this list while hidden. Include both Tensorlake key fields because they hold separate
    /// drafts, although a successful write changes the same stored account.
    fn edits(&self, cx: &App) -> Vec<Edit> {
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
        if let Some(batch) = &self.batch {
            for edit in &batch.active {
                if !edits.iter().any(|current| current.id() == edit.id()) {
                    edits.push(edit.clone());
                }
            }
        }
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
        self.start_batch(Some(self.shown.clone()), window, cx);
    }

    /// The current page has only one Save even when it holds several fields, as Checkouts does. Its identity
    /// survives the first field settling so keyboard focus stays on Save until the batch ends.
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
            .busy(self.batch.is_some() || saving || edits.iter().any(|edit| edit.holds_save(cx)))
            .on_click(move |_, window, cx| {
                let _ = dialog.update(cx, |dialog, cx| dialog.save_page(window, cx));
            }),
        );
        Some((name, answers))
    }

    fn start_batch(&mut self, page: Option<Section>, window: &mut Window, cx: &mut Context<Self>) {
        if self.batch.is_some() {
            return;
        }
        let mut edits = self.edits(cx);
        self.key_conflict = false;
        if let Some(page) = &page {
            edits.retain(|edit| edit.is_form() && &edit.section() == page);
        } else if self.keys_disagree(cx) {
            self.reveal_key_conflict(window, cx);
            return;
        }

        let (saving, dirty): (Vec<_>, Vec<_>) = edits
            .into_iter()
            .partition(|edit| edit.state(cx) == EditState::Saving);
        self.save_generation = self.save_generation.wrapping_add(1);
        let generation = self.save_generation;
        self.batch = Some(Batch {
            generation,
            close: page.is_none(),
            check_keys: page.is_none(),
            active: saving.clone(),
            remaining: dirty.into(),
            paired_key: None,
            after: AfterBatch::None,
        });
        for edit in saving {
            self.dispatch_edit(&edit, generation, true, window, cx);
        }
        self.advance_batch(window, cx);
    }

    /// Both forms write the same key. Refuse the close batch before any writes if the two drafts disagree.
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

    fn dispatch_edit(
        &mut self,
        edit: &Edit,
        generation: u64,
        already_saving: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dialog = cx.weak_entity();
        let handle = window.window_handle();
        let id = edit.id();
        let started = edit.save(already_saving, window, cx, move |saved, cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = dialog.update(cx, |dialog, cx| {
                    dialog.edit_finished(generation, id, saved, window, cx)
                });
            });
        });
        if !started {
            self.edit_finished(generation, id, false, window, cx);
        }
    }

    /// Only the batch that dispatched this target can consume its receipt.
    fn edit_finished(
        &mut self,
        generation: u64,
        id: EntityId,
        saved: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(batch) = self
            .batch
            .as_mut()
            .filter(|batch| batch.generation == generation)
        else {
            return;
        };
        let Some(index) = batch.active.iter().position(|active| active.id() == id) else {
            return;
        };
        let edit = batch.active.remove(index);
        if saved {
            self.advance_batch(window, cx);
        } else {
            if let Some(Batch {
                after: AfterBatch::Question(layer),
                ..
            }) = self.batch.take()
            {
                layers::close(layer, window, cx);
            }
            self.reveal_edit_failure(&edit, window, cx);
        }
    }

    fn advance_batch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Some(batch) = self.batch.as_mut().filter(|batch| batch.active.is_empty()) {
            let generation = batch.generation;
            let Some(next) = batch.remaining.pop_front() else {
                if let Some(batch) = self.batch.take() {
                    match batch.after {
                        AfterBatch::SaveAndClose => self.start_batch(None, window, cx),
                        AfterBatch::Question(layer) => {
                            self.refresh_close_question(layer, window, cx)
                        }
                        AfterBatch::None if batch.close && self.edits(cx).is_empty() => {
                            self.finish_close(window, cx);
                        }
                        AfterBatch::None => {}
                    }
                }
                break;
            };
            if self.edits(cx).iter().any(|edit| edit.id() == next.id()) {
                if let Edit::Key(_, view) = &next
                    && self.batch.as_ref().is_some_and(|batch| batch.check_keys)
                {
                    let paired_key = self
                        .batch
                        .as_ref()
                        .and_then(|batch| batch.paired_key.as_deref());
                    if self.keys_disagree(cx)
                        || paired_key.is_some_and(|approved| {
                            view.read(cx)
                                .typed_key()
                                .is_some_and(|replacement| replacement != approved)
                        })
                    {
                        if let Some(Batch {
                            after: AfterBatch::Question(layer),
                            ..
                        }) = self.batch.take()
                        {
                            layers::close(layer, window, cx);
                        }
                        self.reveal_key_conflict(window, cx);
                        break;
                    }
                    if let Some((key, _)) = self.key_drafts(cx)
                        && let Some(batch) = &mut self.batch
                        && batch.paired_key.is_none()
                    {
                        batch.paired_key = Some(key.to_owned());
                    }
                }
                if let Some(batch) = &mut self.batch {
                    batch.active.push(next.clone());
                }
                self.dispatch_edit(
                    &next,
                    generation,
                    next.state(cx) == EditState::Saving,
                    window,
                    cx,
                );
                break;
            }
        }
        cx.notify();
    }

    fn show_credential_key(&mut self, cx: &mut Context<Self>) {
        if let Some(credentials) = &self.credentials {
            credentials.update(cx, credentials::Credentials::reveal_key);
        }
    }

    /// A failed key write can belong to the Credentials overview even while its separate editor is open.
    fn reveal_edit_failure(&mut self, edit: &Edit, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(edit, Edit::Key(Place::Credentials, _)) {
            self.show_credential_key(cx);
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

    fn finish_close(&self, window: &mut Window, cx: &mut App) {
        if let Some(layer) = self.layer {
            layers::close(layer, window, cx);
        }
    }

    /// All three close routes ask the same question about the loaded drafts, including the ones hidden by search
    /// or another section. Showing a question during a pending save joins that save without sending it again.
    pub(super) fn ask_to_close(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Dismissal {
        let edits = self.edits(cx);
        if edits.is_empty() {
            return Dismissal::Close;
        }
        // A fresh question may change the close decision. The edits already authorized to save keep going.
        if let Some(batch) = &mut self.batch {
            batch.close = false;
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
        let pending: Vec<_> = edits
            .iter()
            .filter(|edit| edit.state(cx) == EditState::Saving)
            .collect();
        let mut question = Question::new(
            "Save changes before closing?",
            Answer::new(
                "Save",
                HoverHelp::tooltip("Save all the listed changes, then close Settings"),
                Tier::Primary,
            ),
            (),
        )
        .line(format!("Unsaved: {}", names.join(", ")))
        .cancel_answer(Answer::new(
            "Keep editing",
            HoverHelp::tooltip("Stay in Settings with this draft"),
            Tier::Neutral,
        ));
        if pending.is_empty() {
            let discard = cx.weak_entity();
            question = question.alternative(
                Answer::new(
                    "Discard",
                    HoverHelp::tooltip("Close Settings without saving the drafts"),
                    Tier::Ghost,
                ),
                move |(), window, cx| {
                    if let Some(layer) = discard.upgrade().and_then(|dialog| dialog.read(cx).layer)
                    {
                        layers::close(layer, window, cx);
                    }
                },
            );
        }
        let save = cx.weak_entity();
        let mark = Mark::take(cx);
        if let Some(layer) = prompt::ask(
            question,
            None,
            &mark,
            move |(), window, cx| {
                let _ = save.update(cx, |dialog, cx| {
                    if let Some(batch) = &mut dialog.batch {
                        batch.after = AfterBatch::SaveAndClose;
                    } else {
                        dialog.start_batch(None, window, cx);
                    }
                });
            },
            window,
            cx,
        ) {
            if let Some(batch) = &mut self.batch {
                batch.after = AfterBatch::Question(layer);
            } else {
                for edit in pending {
                    self.watch_close_question(edit, layer, window, cx);
                }
            }
        }
        Dismissal::Stay
    }

    /// Reopen the question with its current answers once the operations it originally named have settled.
    fn refresh_close_question(
        &mut self,
        layer: LayerId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if layers::is_open(layer, window, cx) {
            layers::close(layer, window, cx);
            if self.ask_to_close(window, cx) == Dismissal::Close {
                self.finish_close(window, cx);
            }
        }
    }

    /// A pending save may settle before the person answers the question. Once they dismiss it, that outcome no longer
    /// owns a close. An unanswered question closes the window's dialog only once all of its drafts have been saved.
    fn watch_close_question(
        &self,
        edit: &Edit,
        layer: LayerId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let handle = window.window_handle();
        let dialog = cx.weak_entity();
        let section = edit.section();
        let credential_key = matches!(edit, Edit::Key(Place::Credentials, _));
        let rejected = match edit {
            Edit::Recipe(editor) => Some(editor.downgrade()),
            _ => None,
        };
        edit.save(true, window, cx, move |saved, cx| {
            let _ = handle.update(cx, |_, window, cx| {
                if !layers::is_open(layer, window, cx) {
                    return;
                }
                let _ = dialog.update(cx, |dialog, cx| {
                    if !saved {
                        layers::close(layer, window, cx);
                        if credential_key {
                            dialog.show_credential_key(cx);
                        }
                        dialog.reveal_save_failure(section, window, cx);
                        if let Some(editor) = rejected.and_then(|editor| editor.upgrade()) {
                            let save = editor.read(cx).save_focus();
                            crate::focus::focus_in_layer(&save, &Mark::take(cx), window, cx);
                        }
                    } else if dialog
                        .edits(cx)
                        .iter()
                        .all(|edit| edit.state(cx) != EditState::Saving)
                    {
                        dialog.refresh_close_question(layer, window, cx);
                    }
                });
            });
        });
    }
}

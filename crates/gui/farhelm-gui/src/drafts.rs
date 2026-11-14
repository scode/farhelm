//! Drafts: what the user typed into an editor over pushed state, kept apart from what the helm pushes.
//!
//! SPEC_impl.md, "Drafts". An editor opens on a value from the store (a session's title, a template's fields, a
//! preference) and the helm keeps pushing changes while it is open: another client renames the session, an agent
//! edits the template. A [`Draft`] records whether
//! the user touched its field. Until they do, it follows pushed changes; once they have, a pushed change never
//! overwrites what they typed. The old app got there by freezing the seed when the editor opened (`rename.rs` at
//! reference commit `24bc0580`), which also stopped an untouched field from following; following is the rule now.
//!
//! Saving keeps today's API semantics: the draft's value goes to the helm exactly as it stands, through the route's
//! own [`Command`] constructor, and nothing here validates it. The save runs as an operation ([`crate::operations`]),
//! so it outlives the editor's view, and [`save`] settles the draft before the editor's own completion runs, so no
//! editor can forget to. A failed save is reported and never retried or treated as saved: the draft keeps its text
//! and its touched mark, and holds the refusal for the editor to show. That is the Rename editor's rule (SPEC.md: "a
//! refusal leaves its draft and error visible for correction"); an editor whose SPEC rule reverts on failure reloads
//! its draft itself.
//!
//! An untouched draft holds the raw value it was loaded with, so saving it untouched sends back the helm's own bytes,
//! hidden characters included; an escaped presentation never becomes a saved value (SPEC_impl.md, "Host-supplied
//! text").
//!
//! Drafts live in the editor that owns them and do not survive a reset: the reset closes every dialog (the layer
//! manager's `close_all`) and drops the UI session's operations, so neither the draft nor its pending save reaches the
//! rebuilt window (SPEC_impl.md, "One failure path: the reset").
//!
//! A [`Draft`] is framework-free and holds no text field. A text field over a draft is a [`DraftInput`], which binds
//! the two so no editor wires them by hand: it mirrors the field's edits into the draft (putting the field back when a
//! save in flight refuses an edit), follows a pushed value into the field while the draft allows, holds the field
//! read-only while its save is in flight, and after a refused save takes up the pushed value again if the user had not
//! touched the field. Forgetting any of those legs in one editor is the bug "a refresh changed what the user was
//! typing", or its mirror image, a field showing text the draft does not hold, and settings alone has dozens of fields.
//! The editor still reads its own value from the store (each editor knows where its field's value lives) and hands
//! every push to [`DraftInput::follow`], for as long as it is open.
//!
//! What the Rename editor does when its save settles is in [`Settled::user_moved_on`]'s docs: a successful save closes
//! the editor even when the user moved on, and focus goes back to the opener only if it is still inside the editor.
//!
//! Extension points (`docs/gpui/foundation-ownership.md`): a new editor holds a [`Draft`] per field and saves through
//! [`save`], or a [`DraftInput`] per text field and saves through [`DraftInput::save`]. Partial saves are not built:
//! they are used only where the API already supports them or where SPEC.md's "a GUI write must not undo a change an
//! agent or the command line made" needs them, and the first editor with such a route adds them here.

use farhelm_helm_client::PathError;
use gpui::{AnyWindowHandle, App, Context, Subscription, Window};

use crate::components::{TextInputEvent, TextInputState};
use crate::host_text::HostText;
use crate::operations::{Command, NotStarted, OpKey, Outcome, Settled};
use crate::session;

/// One editable field over a pushed value: its current value, whether the user touched it, and its save's state.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft<T> {
    value: T,
    touched: bool,
    saving: bool,
    refusal: Option<HostText>,
}

impl<T: Clone + PartialEq> Draft<T> {
    /// A draft of `value` as the store holds it now, untouched.
    pub fn load(value: T) -> Draft<T> {
        Draft {
            value,
            touched: false,
            saving: false,
            refusal: None,
        }
    }

    /// The field's value: what the user typed once touched, the pushed value otherwise.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Whether the user changed the field since it was loaded or last saved.
    pub fn is_touched(&self) -> bool {
        self.touched
    }

    /// Whether a save is in flight. The editor's field is read-only meanwhile, so the value on screen stays the one
    /// sent, and its Save is busy ([`crate::components::Button::busy`]: it keeps focus and ignores presses).
    pub fn is_saving(&self) -> bool {
        self.saving
    }

    /// The helm's words for the last failed save, until the next save starts.
    pub fn refusal(&self) -> Option<&HostText> {
        self.refusal.as_ref()
    }

    /// The user changed the field to `value`. Marks it touched even when `value` equals what was loaded: someone who
    /// retypes a title means it, and no comparison can tell that apart from not editing. Returns whether the draft
    /// took the edit.
    ///
    /// While a save is in flight the edit is refused, and the field is put back to [`Draft::value`] ([`DraftInput`]
    /// does that for a text field). The field is read-only meanwhile ([`DraftInput`] holds it so), but an input event
    /// can be queued before that takes effect, as the old app found (`rename.rs` at reference commit `24bc0580`).
    /// Taking it would be worse than losing a keystroke: the save's success would clear the touched mark over text that
    /// was never sent, and the next push would overwrite it.
    #[must_use = "a refused edit leaves the input showing text the draft does not hold"]
    pub fn edit(&mut self, value: T) -> bool {
        if self.saving {
            return false;
        }
        self.value = value;
        self.touched = true;
        true
    }

    /// The helm pushed `pushed` for this field. An untouched field takes it; a touched one keeps what the user typed,
    /// and so does a field whose save is in flight, whose value on screen must stay the one sent. Returns whether the
    /// value changed, so the editor knows to update its input.
    pub fn follow(&mut self, pushed: T) -> bool {
        if self.touched || self.saving || self.value == pushed {
            return false;
        }
        self.value = pushed;
        true
    }

    /// Record how the save ended. A done save counts as saved and clears the touched mark, so later pushes are
    /// followed again; a refused one keeps the text and the mark and records the helm's words.
    fn settle(&mut self, outcome: &Outcome) {
        self.saving = false;
        match outcome {
            Outcome::Done(_) => self.touched = false,
            Outcome::Refused(message) => self.refusal = Some(message.clone()),
        }
    }
}

/// Save one draft of `view` as work on `key` (the session, template, host or preferences it saves): send `command`
/// built from the draft's value, then, once the helm answers, settle the draft and call `then` with the outcome.
///
/// `field` reaches the draft inside the view; it holds nothing of the view (a plain function, or a closure over one) so
/// the save can reach the same draft again when the reply arrives. A second Save while the first is in flight (Enter
/// pressed again) is refused by the operation store, like any work on a key with work in flight. Nothing is retried,
/// and if the view is gone when the reply arrives, only the operation store's record of a refusal remains under `key`
/// (on a session's row, for a session).
pub fn save<V: 'static, T: Clone + PartialEq + 'static>(
    view: &mut V,
    field: impl Fn(&mut V) -> &mut Draft<T> + Copy + 'static,
    key: &OpKey,
    command: impl FnOnce(&T) -> Result<Command, PathError>,
    cx: &mut Context<V>,
    then: impl FnOnce(&mut V, &Settled, &mut Context<V>) + 'static,
) -> Result<(), NotStarted> {
    let command = command(&field(view).value).map_err(NotStarted::Path)?;
    let operations = session::current_operations(cx).ok_or(NotStarted::Retired)?;
    let editor = cx.entity().downgrade();
    operations.update(cx, |operations, cx| {
        operations.start(key, command, cx, move |settled, cx| {
            let _ = editor.update(cx, |view, cx| {
                field(view).settle(&settled.outcome);
                then(view, settled, cx);
                cx.notify();
            });
        })
    })?;
    let draft = field(view);
    draft.saving = true;
    draft.refusal = None;
    cx.notify();
    Ok(())
}

// ---- A text field bound to a draft ----------------------------------------------------------------------------------

/// A text field over a [`Draft<String>`], kept in step with it: what the module docs say an editor must otherwise wire
/// by hand for each field. The editor holds one per text field, draws [`DraftInput::input`] with
/// [`crate::components::TextInput`], hands it every pushed value ([`DraftInput::follow`]) and saves through
/// [`DraftInput::save`].
pub struct DraftInput {
    draft: Draft<String>,
    input: TextInputState,
    /// The value the helm last pushed for the field: what an untouched draft takes up again after a refused save.
    pushed: String,
    _events: Subscription,
}

impl DraftInput {
    /// A field showing `value` (the raw value from the store) over a draft of it, untouched. `field` reaches this
    /// within the editor `V`; `on_submit` runs when the user presses Enter in the field (an editor's Save, usually).
    pub fn new<V: 'static>(
        value: &str,
        input: TextInputState,
        field: fn(&mut V) -> &mut DraftInput,
        on_submit: fn(&mut V, &mut Window, &mut Context<V>),
        window: &mut Window,
        cx: &mut Context<V>,
    ) -> DraftInput {
        input.set_text(value.to_owned(), window, cx);
        let events = input.subscribe(window, cx, move |view, event, window, cx| match event {
            TextInputEvent::Changed => field(view).edited(window, cx),
            TextInputEvent::Submitted => on_submit(view, window, cx),
        });
        DraftInput {
            draft: Draft::load(value.to_owned()),
            input,
            pushed: value.to_owned(),
            _events: events,
        }
    }

    /// The draft: its value, whether it is touched or saving, and the last refusal.
    pub fn draft(&self) -> &Draft<String> {
        &self.draft
    }

    /// The field's state, for the editor to draw.
    pub fn input(&self) -> &TextInputState {
        &self.input
    }

    /// The helm pushed `pushed` for this field: the field shows it if the draft follows it ([`Draft::follow`]: only
    /// while untouched and not saving). Setting the field raises no change event, so following never marks it touched.
    pub fn follow(&mut self, pushed: &str, window: &mut Window, cx: &mut App) {
        self.pushed = pushed.to_owned();
        if self.draft.follow(pushed.to_owned()) {
            self.input.set_text(self.draft.value().clone(), window, cx);
        }
    }

    /// The user changed the field: mirror it into the draft. While a save is in flight the field is read-only, so an
    /// edit arrives then only if it was queued before the field became read-only; the draft refuses it
    /// ([`Draft::edit`]) and the field is put back to the value being saved. That moves the caret to the end and clears
    /// the field's undo history, which is why read-only is the main guard and this only the backstop.
    fn edited<V: 'static>(&mut self, window: &mut Window, cx: &mut Context<V>) {
        let text = self.input.text(cx).to_string();
        if !self.draft.edit(text) {
            self.input.set_text(self.draft.value().clone(), window, cx);
        }
        cx.notify();
    }

    /// Save the field's draft as work on `key`, as [`save`] does, holding the field read-only until the helm answers.
    /// After a refusal an untouched draft, which did not follow pushes while saving, takes up the last pushed value:
    /// saving it again must not send back a value someone else has since replaced. A touched draft keeps what the user
    /// typed. Then `then` runs with the outcome.
    pub fn save<V: 'static>(
        view: &mut V,
        field: fn(&mut V) -> &mut DraftInput,
        key: &OpKey,
        command: impl FnOnce(&str) -> Result<Command, PathError>,
        window: &mut Window,
        cx: &mut Context<V>,
        then: impl FnOnce(&mut V, &Settled, &mut Context<V>) + 'static,
    ) -> Result<(), NotStarted> {
        let handle = window.window_handle();
        let started = save(
            view,
            move |view| &mut field(view).draft,
            key,
            |value: &String| command(value),
            cx,
            move |view, settled, cx| {
                let input = field(view);
                input.input.set_read_only(false, cx);
                if let Outcome::Refused(_) = settled.outcome {
                    input.follow_after_refusal(handle, cx);
                }
                then(view, settled, cx);
            },
        );
        if started.is_ok() {
            // The text on screen must stay the text sent, and read-only keeps the caret and undo history where they
            // are. `edited` still undoes an edit that was queued before this took effect.
            field(view).input.set_read_only(true, cx);
        }
        started
    }

    /// Take up the last pushed value after a refused save, if the draft follows it. The field is set once the
    /// completion has returned, since a completion has no window at hand.
    fn follow_after_refusal<V: 'static>(&mut self, window: AnyWindowHandle, cx: &mut Context<V>) {
        if !self.draft.follow(self.pushed.clone()) {
            return;
        }
        let input = self.input.clone();
        let value = self.draft.value().clone();
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| input.set_text(value, window, cx));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule the module exists for: a pushed change never overwrites a field the user touched, or the title
    /// someone is halfway through typing vanishes when another client renames the session. Specified: an untouched
    /// draft follows a push; once edited it keeps its value through later pushes, even an edit back to the pushed
    /// spelling.
    #[test]
    fn a_pushed_change_never_overwrites_a_touched_field() {
        let mut draft = Draft::load("loaded".to_owned());
        assert!(draft.follow("pushed".to_owned()), "untouched follows");
        assert_eq!(draft.value(), "pushed");
        assert!(
            !draft.follow("pushed".to_owned()),
            "an equal push changes nothing"
        );

        assert!(draft.edit("pushed".to_owned()));
        assert!(
            draft.is_touched(),
            "retyping the same text still touches it"
        );
        assert!(draft.edit("typed".to_owned()));
        assert!(!draft.follow("pushed again".to_owned()));
        assert_eq!(
            draft.value(),
            "typed",
            "the push did not overwrite the edit"
        );
    }

    /// While a save is in flight, the value on screen is the one sent. A push that replaced it would show a title the
    /// helm is not saving, and an edit taken then would be lost: the save's success clears the touched mark over text
    /// that was never sent. Specified: a saving draft refuses edits and ignores pushes, keeping the value sent, and
    /// takes both again once its save is done.
    #[test]
    fn a_saving_draft_takes_no_edit_and_no_push_until_its_save_is_done() {
        let mut draft = Draft::load("sent".to_owned());
        draft.saving = true;
        assert!(
            !draft.edit("typed meanwhile".to_owned()),
            "the edit is refused"
        );
        assert!(!draft.follow("pushed".to_owned()), "the push is ignored");
        assert_eq!(draft.value(), "sent");
        assert!(!draft.is_touched());
        draft.settle(&Outcome::Done(None));
        assert!(draft.follow("pushed".to_owned()));
        assert!(draft.edit("typed".to_owned()));
    }

    /// A failed save must not be treated as saved: SPEC.md's Rename rule keeps the draft and its error visible for
    /// correction. Specified: a refusal keeps the value and the touched mark and records the helm's words; a done save
    /// clears the mark.
    #[test]
    fn only_a_done_save_counts_as_saved() {
        let mut draft = Draft::load("old".to_owned());
        assert!(draft.edit("bad\ntitle".to_owned()));
        draft.saving = true;
        draft.settle(&Outcome::Refused(HostText::from_host(
            "titles are one line",
        )));
        assert_eq!(draft.value(), "bad\ntitle");
        assert!(draft.is_touched() && !draft.is_saving());
        assert_eq!(
            draft.refusal(),
            Some(&HostText::from_host("titles are one line"))
        );

        draft.settle(&Outcome::Done(None));
        assert!(!draft.is_touched(), "done is saved");
    }
}

//! Drafts: what the user typed into an editor over pushed state, kept apart from what the helm pushes.
//!
//! SPEC_impl.md, "Drafts". An editor opens on a value from the store (a session's title, say) and the helm keeps
//! pushing changes while it is open: another client renames the session, an agent does. A [`Draft`] records whether
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
//! NOTE: The draft is framework-free and does not hold the text input's state. The editor mirrors the input into it:
//! on the input's change event it calls [`Draft::edit`] with the input's text (and puts the input back to
//! [`Draft::value`] when the edit is refused because a save is in flight); on a pushed change it calls
//! [`Draft::follow`] and, when that returns true, sets the input's text (`TextInputState::set_text`, which raises no
//! change event, so following never marks the field touched). An editor watches its session's store for those pushes
//! for as long as it is open.
//!
//! What the Rename editor does when its save settles is in [`Settled::user_moved_on`]'s docs: a successful save closes
//! the editor even when the user moved on, and focus goes back to the opener only if it is still inside the editor.
//!
//! Extension points (`docs/gpui/foundation-ownership.md`): a new editor holds a [`Draft`] per field and saves through
//! [`save`]. Partial saves are not built: they are used only where the API already supports them or where SPEC.md's
//! "a GUI write must not undo a change an agent or the command line made" needs them, and the first editor with such a
//! route adds them here.

use farhelm_helm_client::PathError;
use gpui::Context;

use crate::host_text::HostText;
use crate::operations::{Command, NotStarted, Outcome, Settled};
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

    /// Whether a save is in flight. The editor draws its field read-only and its Save busy meanwhile, so the value
    /// on screen stays the one sent.
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
    /// While a save is in flight the edit is refused, and the editor puts its input back to [`Draft::value`]. The
    /// editor draws the field read-only meanwhile, but an input event can be queued before that draw, as the old app
    /// found (`rename.rs` at reference commit `24bc0580`). Taking it would be worse than losing a keystroke: the
    /// save's success would clear the touched mark over text that was never sent, and the next push would overwrite it.
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

/// Save one draft of `view` on `session`: send `command` built from the draft's value, then, once the helm
/// answers, settle the draft and call `then` with the outcome.
///
/// `field` reaches the draft inside the view; it is a plain function so the save can reach the same draft again when
/// the reply arrives. A second Save while the first is in flight (Enter pressed again) is refused by the operation
/// store, like any work on a session with work in flight. Nothing is retried, and if the view is gone when the reply
/// arrives, only the operation store's record of a refusal remains, on the session's row.
pub fn save<V: 'static, T: Clone + PartialEq + 'static>(
    view: &mut V,
    field: fn(&mut V) -> &mut Draft<T>,
    session: &str,
    command: impl FnOnce(&T) -> Result<Command, PathError>,
    cx: &mut Context<V>,
    then: impl FnOnce(&mut V, &Settled, &mut Context<V>) + 'static,
) -> Result<(), NotStarted> {
    let command = command(&field(view).value).map_err(NotStarted::Path)?;
    let operations = session::current_operations(cx).ok_or(NotStarted::Retired)?;
    let editor = cx.entity().downgrade();
    operations.update(cx, |operations, cx| {
        operations.start(session, command, cx, move |settled, cx| {
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

//! Leaving the recipe editor, and deleting its recipe: the question "Save these changes before leaving?" that Back,
//! Cancel and closing the hosting dialog ask while the draft has unsaved changes, and Delete with its confirmation.
//!
//! SPEC.md, "Topology" (the Recipes paragraphs: "Leaving it with unsaved changes, whether by its own Back or Cancel or
//! by closing Settings with Escape or a close control, asks whether to save, discard or keep editing, and saving or
//! discarding then completes the close. Delete asks for confirmation"; "Its actions and leave/delete questions and
//! refusal messages stay visible at the bottom of the scrolling panel"). Ported from the old editor's leave and delete
//! questions and its `DialogCloseGuard` (`tensorlake/recipe_editor.rs` at reference commit `24bc0580`), with their
//! words. Here's the TLDR of its rules:
//!
//! - **The editor owns its leaving.** Back and Cancel call [`Editor::leave`] for a return to the list, and the hosting
//!   dialog's dismissal hook calls [`Editor::dismissal`] for a close, so a dialog other than Settings can host the
//!   editor with the same rules. Where a departure goes is the section's ([`super::super::Recipes::depart`]).
//! - **The questions are the prompt's, inline.** Each is drawn where the editor's answers are, in their place, as the
//!   old questions replaced the ordinary actions; Keep editing, Discard and Save in the old order, Cancel and Delete for
//!   the delete confirmation. Opening one takes the other away.
//! - **Save finishes the leave.** Save in the question saves as Save does, and the done answer completes the departure
//!   it was asked for; a refused save keeps the editor open with the helm's words, as any refused save does. Discard
//!   leaves at once. The question goes when it is answered, so a refused save shows the ordinary answers again under
//!   its words, where the old question stayed up through the save and after its refusal (the prompt's inline Save runs
//!   once; recorded in `docs/gpui/parity/settings-and-hosts.md`, "Deviations").
//! - **A close is asked about even while a save is out.** Back and Cancel are unavailable while a save or a delete is
//!   out, as in the old editor. A close still asks about unsaved changes then, as the old one did so that a close
//!   never silently waited or dropped the draft. The question then offers Keep editing and Save only: the draft on
//!   screen is the one being saved, so a Discard could not keep its word, and the prompt cannot disable one answer
//!   (the old one disabled every answer until the save ended). Save waits for the save already out, and that save's
//!   done answer finishes the close even while the question is still unanswered, as the old pending close did. A
//!   close while a delete is out closes at once: the recipe is going, and the delete goes on as a save does. Both are
//!   recorded in `docs/gpui/parity/settings-and-hosts.md`, "Deviations".
//! - **Delete is the recipe's own work.** `DELETE /api/tensorlake/recipes/<name>`, under the recipe's key, as Save. A
//!   done answer closes the editor; the helm has pushed the recipe gone. A refusal the helm gives at once (the recipe in
//!   use, or already being deleted) stays in the editor and leaves the key, as Save's do. A cleanup still running at the
//!   helm's 20-second bound (a 504 saying the deletion carries on) closes the editor too, and its words stay on the
//!   recipe's key, so the row says the deletion carries on until the push takes the recipe away, or, if the cleanup
//!   fails, says to delete it again to see why; the old app had no such bound and waited (decided by this area on
//!   2026-11-15, recorded in `docs/gpui/parity/settings-and-hosts.md`, "Deviations"). The status alone tells the two
//!   apart: the helm answers any cleanup failure of the timeout kind with a 504 too, so such a failure inside the
//!   bound also closes the editor, and the row then shows the helm's own words for it while the recipe stays listed.
//! - **The words outlive the recipe on its key, harmlessly.** Once the recipe leaves the list its key keeps the 504's
//!   words, drawn under no row. A recipe created again under that name cannot show them: every create in this app goes
//!   through the editor's Save on the same key, and a start on a key clears its refusal. So nothing forgets the key
//!   when the recipe leaves (the migration's orchestrator decided on 2026-11-15; a memo of listed names would also miss
//!   a recipe that left while Settings was closed).

use farhelm_helm_client::Method;
use gpui::{App, Context, Window};

use super::{Editor, path};
use crate::components::{HoverHelp, Tier};
use crate::focus::Mark;
use crate::layers::Dismissal;
use crate::operations::{Command, NotStarted, OpKey, Outcome, Work};
use crate::prompt::{self, Answer, Question};
use crate::session;

/// The lead-in for a Delete confirmed while the recipe's key has other work in flight, before the store's shared reason,
/// which every area gives after a lead-in of its own (host removal's is "Not started: ").
const BUSY_LEAD_IN: &str = "Not deleted: ";

/// The helm's status for a delete whose cleanup did not finish within its reply bound and carries on.
const CARRIES_ON: u16 = 504;

/// Where leaving the editor goes: back to the list, or out of the hosting dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::settings) enum Departure {
    Back,
    Close,
}

/// One answer's button.
fn answer(label: &'static str, help: &'static str, tier: Tier) -> Answer {
    Answer {
        label: label.into(),
        help: HoverHelp::tooltip(help),
        tier,
    }
}

impl Editor {
    /// Whether a save or a delete is out, when nothing may change the form or leave it.
    pub(super) fn busy(&self) -> bool {
        self.saving || self.deleting
    }

    /// Back or Cancel: return to the list, at once when nothing is unsaved, or else after the leave question. Nothing
    /// while a save or delete is out.
    pub(super) fn leave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        if self.dirty(cx) {
            self.ask_leave(Departure::Back, window, cx);
        } else {
            let mark = Mark::take(cx);
            self.depart(Departure::Back, &mark, Some(window), cx);
        }
    }

    /// Whether closing the hosting dialog now asks the leave question rather than closing: the form has unsaved
    /// changes and no delete is out (the module docs). The host asks before [`Editor::dismissal`], to show the editor
    /// first when it is hidden.
    pub(in crate::settings) fn asks_before_leaving(&self, cx: &App) -> bool {
        !self.deleting && self.dirty(cx)
    }

    /// What the hosting dialog's dismissal hook answers for the editor (Escape and its close controls): stay and ask
    /// the leave question when [`Editor::asks_before_leaving`], and otherwise let the dialog close.
    pub(in crate::settings) fn dismissal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Dismissal {
        if self.asks_before_leaving(cx) {
            self.ask_leave(Departure::Close, window, cx);
            return Dismissal::Stay;
        }
        Dismissal::Close
    }

    /// Go where `departure` leads, for the user's action at `mark`.
    fn depart(
        &mut self,
        departure: Departure,
        mark: &Mark,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let id = cx.entity_id();
        let _ = self.parent.update(cx, |parent, cx| {
            parent.depart(id, departure, mark, window, cx)
        });
    }

    /// A save the editor sent answered done: leave as the leave question's Save asked (`leaving`), or where the leave
    /// question still open asks to go (a close asked while the save was out), or else back to the list, for the press
    /// at `mark` that sent it. Keep editing has closed the question by then, so it falls back to the list.
    pub(super) fn saved(&mut self, mark: &Mark, cx: &mut Context<Self>) {
        let asked = self
            .leave_question
            .as_ref()
            .filter(|(_, question)| question.is_open(cx))
            .map(|(departure, _)| (*departure, *mark));
        let (departure, mark) = self
            .leaving
            .take()
            .or(asked)
            .unwrap_or((Departure::Back, *mark));
        self.depart(departure, &mark, None, cx);
    }

    /// Ask "Save these changes before leaving?" for `departure` in the answers' place, replacing any question up. No
    /// Discard while a save is out (the module docs).
    fn ask_leave(&mut self, departure: Departure, window: &mut Window, cx: &mut Context<Self>) {
        let mark = Mark::take(cx);
        let mut question = Question::new(
            "Save these changes before leaving?",
            answer("Save", "Save these changes, then go back", Tier::Primary),
            departure,
        )
        .cancel_answer(answer(
            "Keep editing",
            "Stay here with the unsaved changes",
            Tier::Neutral,
        ));
        if !self.saving {
            let discard = cx.weak_entity();
            question = question.alternative(
                answer(
                    "Discard",
                    "Discard these changes and go back",
                    Tier::Neutral,
                ),
                move |departure, window, cx| {
                    let _ = discard.update(cx, |editor, cx| {
                        let mark = Mark::take(cx);
                        editor.depart(departure, &mark, Some(window), cx)
                    });
                },
            );
        }
        let save = cx.weak_entity();
        let inline = prompt::ask_inline(
            question,
            &mark,
            move |departure, window, cx| {
                let _ = save.update(cx, |editor, cx| {
                    let mark = Mark::take(cx);
                    // The fields stay editable while the question is up: with nothing left unsaved, leave at once. A
                    // close asked while a save was out waits for that save instead of sending another.
                    if !editor.dirty(cx) {
                        editor.depart(departure, &mark, Some(window), cx);
                    } else if editor.saving || editor.save(cx) {
                        editor.leaving = Some((departure, mark));
                    }
                });
            },
            window,
            cx,
        );
        // The newest question owns the decision: a departure an earlier question's Save asked for, its save still
        // out, gives way to this one's answer (or to this one's departure while it is unanswered).
        self.leaving = None;
        self.delete_question = None;
        self.leave_question = Some((departure, inline));
        cx.notify();
    }

    /// Delete: ask the old confirmation in the answers' place, replacing any question up. Nothing for a recipe not
    /// saved yet, or while a save or delete is out.
    pub(super) fn ask_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.saved.clone() else {
            return;
        };
        if self.busy() {
            return;
        }
        let mark = Mark::take(cx);
        // A build may finish while the editor stays open, so the words warn about every place's cleanup whatever the
        // row showed: only Farhelm-built artifacts belong to the recipe, and containers can still keep its images in
        // use.
        let question = Question::new(
            format!(
                "Delete recipe {name}? Running builds are cancelled and Farhelm's images and snapshots for this \
                 recipe are deleted. Images in use must be released first."
            ),
            answer("Delete", "Delete this recipe", Tier::Danger),
            name,
        )
        // SPEC.md, "Opening and closing": a risky action that cannot be undone starts on Cancel. Deleting cancels the
        // recipe's builds and deletes its snapshots and images, which only a billed rebuild brings back.
        .starts_on_cancel();
        let editor = cx.weak_entity();
        let inline = prompt::ask_inline(
            question,
            &mark,
            move |name, _, cx| {
                let _ = editor.update(cx, |editor, cx| editor.delete(name, cx));
            },
            window,
            cx,
        );
        self.leave_question = None;
        self.delete_question = Some(inline);
        cx.notify();
    }

    /// The confirmed delete of the recipe `name` (the module docs).
    fn delete(&mut self, name: String, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let path = match path(Some(&name)) {
            Ok(path) => path,
            Err(error) => {
                tracing::debug!(?error, "recipe delete not built");
                return;
            }
        };
        let Some(operations) = session::current_operations(cx) else {
            return;
        };
        let shown = format!("DELETE {}", path.as_str());
        let command = Command::new(Work::Save, Method::Delete, path, None, shown);
        let key = OpKey::recipe(&name);
        let forgetting = key.clone();
        let mark = Mark::take(cx);
        let editor = cx.weak_entity();
        let started = operations.update(cx, |operations, cx| {
            operations.start(&key, command, cx, move |settled, cx| {
                let refusal = match &settled.outcome {
                    Outcome::Refused(refusal) if refusal.status != CARRIES_ON => {
                        refusal.words.clone()
                    }
                    // Gone, or going: either way the editor has nothing left to edit. A deletion that carries on keeps
                    // its words on the key, where the row shows them until the recipe leaves the list.
                    Outcome::Done(_) | Outcome::Accepted(_) | Outcome::Refused(_) => {
                        let _ = editor.update(cx, |editor, cx| {
                            editor.set_deleting(false, cx);
                            editor.depart(Departure::Back, &mark, None, cx);
                        });
                        return;
                    }
                };
                let shown = editor
                    .update(cx, |editor, cx| {
                        editor.set_deleting(false, cx);
                        let draft = editor.draft(cx);
                        editor.error = Some((refusal, draft));
                        cx.notify();
                    })
                    .is_ok();
                // Shown here, so it leaves the key the row's build shares, as a refused save does.
                if shown && let Some(operations) = session::current_operations(cx) {
                    operations.update(cx, |operations, cx| operations.forget(&forgetting, cx));
                }
            })
        });
        match started {
            Ok(()) => {
                self.error = None;
                self.set_deleting(true, cx);
            }
            // The key's other work is the row's build or rebuild, sent from the list before the editor opened. Said in
            // the error's place rather than on the busy line, whose words are Save's ("Not saved: …").
            Err(NotStarted::Busy(_)) => {
                let draft = self.draft(cx);
                self.fail(
                    format!("{BUSY_LEAD_IN}{}", NotStarted::BUSY_REASON),
                    draft,
                    cx,
                );
            }
            Err(not_started) => tracing::debug!(?not_started, "recipe not deleted"),
        }
        cx.notify();
    }

    /// Mark the delete out or settled, locking the form as a save does.
    fn set_deleting(&mut self, deleting: bool, cx: &mut App) {
        self.deleting = deleting;
        self.lock(cx);
    }
}

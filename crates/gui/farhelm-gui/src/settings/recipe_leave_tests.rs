//! Tests of leaving the recipe editor and deleting its recipe, over the helm client's fake: the question "Save these
//! changes before leaving?" that Back, Cancel, Escape and Settings' close controls ask for unsaved changes, with its
//! Keep editing, Discard and Save, and Delete with its confirmation, its refusals and the helm's bound on how long a
//! deletion's cleanup may take before it answers.
//!
//! The editor is opened as in its own tests (`super::recipe_editor_tests`), whose helpers these share. Every step ends
//! in one of the shared harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::Section;
use super::recipe_editor_tests::{draft, one_command, open_editor_with, show};
use super::recipe_tests::{answer_account, entry, idle};
use super::tests::{available, drawn, fill, label_of, open_settings, section_button, sent};
use crate::operations::{HelmMarker, OpKey, Work};
use crate::session;
use crate::test_support::{any_open, bounds, center, click, focused_id, frame, keys, settle};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The leave question's title, the old editor's words.
const LEAVE: &str = "Save these changes before leaving?";
/// The Settings-wide question also covers drafts in other sections.
const CLOSE: &str = "Save changes before closing?";

/// Settings at Recipes over one saved recipe, `rust-dev`, with its editor open on an empty credential catalog.
fn edit_rust_dev(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext) {
    let (mut helm, cx) = show(cx, vec![entry("rust-dev", idle(), Value::Null)]);
    open_editor_with(&mut helm, cx, "recipe-edit-rust-dev", json!([]));
    (helm, cx)
}

/// The shared prompt's title and available answers, whether it is the leave layer or the inline delete band.
fn question(cx: &mut VisualTestContext) -> Option<(String, Vec<String>)> {
    let title = label_of(cx, "prompt")?;
    let answers = frame(cx)
        .into_iter()
        .filter(|node| {
            matches!(
                node.element_id.as_deref(),
                Some("prompt-cancel" | "prompt-alternative-0" | "prompt-confirm")
            )
        })
        .filter_map(|node| node.label.map(|label| label.to_string()))
        .collect();
    Some((title, answers))
}

/// How a test leaves the editor: a press on a control (the editor's Back or Cancel, or one of Settings' close
/// controls), or Escape.
#[derive(Clone, Copy, Debug)]
enum Leave {
    Press(&'static str),
    Escape,
}

/// Leave the editor `how`. Escape is pressed from the Name field, which the click puts focus in first, so where the
/// key lands does not depend on where an earlier step left focus.
fn leave_by(cx: &mut VisualTestContext, how: Leave) {
    match how {
        Leave::Press(control) => click(cx, control),
        Leave::Escape => {
            click(cx, "recipe-name");
            keys(cx, "escape");
        }
    }
}

/// What the operation store holds on `name`'s key: the work in flight, and the refusal kept.
fn recipe_key(cx: &mut VisualTestContext, name: &str) -> (Option<Work>, Option<String>) {
    cx.update(|_, cx| {
        let operations = session::current_operations(cx).expect("a UI session");
        let operations = operations.read(cx);
        let key = OpKey::recipe(name);
        (
            operations.in_flight(&key),
            operations
                .failure(&key)
                .map(|failure| failure.message.raw().to_owned()),
        )
    })
}

// ---- The question before leaving -----------------------------------------------------------------------------------

/// SPEC.md: "Leaving it with unsaved changes, whether by its own Back or Cancel or by closing Settings with Escape or a
/// close control, asks whether to save, discard or keep editing"; leaving at once would drop work the user typed.
/// Specified: for each of Back, Cancel, Escape, the header's close control and the footer's Close, with a new recipe's
/// name typed, the question opens over the editor with Keep editing, Discard and Save in order, focus on
/// Save, nothing sent and Settings still open; Keep editing takes it away and the draft is as typed.
#[gpui::test]
fn every_way_out_asks_first_and_keep_editing_stays(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, Vec::new());
    open_editor_with(&mut helm, cx, "recipe-new", json!([]));
    fill(cx, "recipe-name", "scratch");
    for how in [
        Leave::Press("recipe-back"),
        Leave::Press("recipe-cancel"),
        Leave::Escape,
        Leave::Press("settings-close"),
        Leave::Press("settings-footer-close"),
    ] {
        leave_by(cx, how);
        let title = match how {
            Leave::Press("recipe-back" | "recipe-cancel") => LEAVE,
            Leave::Escape | Leave::Press(_) => CLOSE,
        };
        assert_eq!(
            question(cx),
            Some((
                title.to_owned(),
                vec!["Keep editing".into(), "Discard".into(), "Save".into()]
            )),
            "{how:?} asks"
        );
        assert!(
            drawn(cx, "recipe-save"),
            "{how:?}: the ordinary footer stays behind the question"
        );
        assert_eq!(
            focused_id(cx).as_deref(),
            Some("prompt-confirm"),
            "{how:?}: focus on Save"
        );
        assert!(
            any_open(cx) && drawn(cx, "recipe-editor"),
            "{how:?}: nothing left"
        );
        if matches!(how, Leave::Escape) {
            keys(cx, "escape");
        } else {
            click(cx, "prompt-cancel");
        }
        assert_eq!(
            question(cx),
            None,
            "{how:?}: Keep editing took the question away"
        );
        assert_eq!(draft(cx).name, "scratch", "{how:?}: the draft is kept");
    }
    assert_eq!(sent(&mut helm), [], "nothing was sent");
}

/// Nothing unsaved means nothing to ask about: the question would only slow the user down. Specified: on an Edit with
/// no change, Back returns to the list at once; opened again, Escape closes Settings at once; neither sends anything.
#[gpui::test]
fn an_unchanged_editor_leaves_without_asking(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    click(cx, "recipe-back");
    assert!(!drawn(cx, "recipe-editor") && drawn(cx, "recipes-list"));
    open_editor_with(&mut helm, cx, "recipe-edit-rust-dev", json!([]));
    keys(cx, "escape");
    assert!(!any_open(cx), "Escape closed Settings");
    assert_eq!(sent(&mut helm), []);
}

/// "Saving or discarding then completes the close": Discard must finish what the user asked for, a return to the list
/// for Back and the dialog's close for a close control, without saving. Specified: Back then Discard shows the list
/// and sends nothing; reopened and changed, Escape then Discard closes Settings and sends nothing.
#[gpui::test]
fn discard_finishes_the_leave_without_saving(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    fill(cx, "recipe-disk", "30");
    click(cx, "recipe-back");
    assert_eq!(question(cx).map(|(title, _)| title).as_deref(), Some(LEAVE));
    assert!(
        !bounds(cx, "prompt").contains(&center(cx, "settings-close")),
        "the host Close is outside the question"
    );
    click(cx, "settings-close");
    assert_eq!(question(cx).map(|(title, _)| title).as_deref(), Some(LEAVE));
    click(cx, "prompt-alternative-0");
    assert!(
        !drawn(cx, "recipe-editor") && drawn(cx, "recipes-list"),
        "back at the list"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("recipe-new"));

    open_editor_with(&mut helm, cx, "recipe-edit-rust-dev", json!([]));
    fill(cx, "recipe-disk", "30");
    leave_by(cx, Leave::Escape);
    assert_eq!(question(cx).map(|(title, _)| title).as_deref(), Some(CLOSE));
    click(cx, "prompt-alternative-0");
    assert!(!any_open(cx), "Settings closed");
    assert_eq!(sent(&mut helm), [], "nothing was saved");
}

/// Save in the question saves as Save does and finishes the leave only once the helm took the save; a refused save
/// must keep the editor open with the helm's words, or the user would lose the draft and never learn why. Specified:
/// the header's close then Save sends the update and leaves Settings open while it is out; its done answer closes
/// Settings. Reopened and changed, Back then Save sends again; a refusal keeps the editor open with the helm's words
/// and the draft, the question gone and the ordinary answers back.
#[gpui::test]
fn the_questions_save_finishes_the_leave_once_the_save_is_done(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    fill(cx, "recipe-disk", "30");
    click(cx, "settings-close");
    click(cx, "prompt-confirm");
    let save = one_command(&mut helm);
    assert_eq!(
        (save.method, save.path.as_str()),
        (Method::Put, "/api/tensorlake/recipes/rust-dev")
    );
    assert!(any_open(cx), "Settings waits for the save");
    helm.reply(save.id, 200, Some(entry("rust-dev", idle(), Value::Null)));
    settle(cx);
    assert!(!any_open(cx), "the done save finished the close");

    open_settings(cx);
    click(cx, section_button(Section::Recipes));
    answer_account(&mut helm, cx, true);
    open_editor_with(&mut helm, cx, "recipe-edit-rust-dev", json!([]));
    fill(cx, "recipe-disk", "40");
    click(cx, "recipe-back");
    click(cx, "prompt-confirm");
    let save = one_command(&mut helm);
    helm.reply(
        save.id,
        409,
        Some(json!("the recipe rust-dev is being deleted")),
    );
    settle(cx);
    assert!(
        drawn(cx, "recipe-editor"),
        "a refused save keeps the editor"
    );
    assert_eq!(
        label_of(cx, "recipe-editor-error").as_deref(),
        Some("the recipe rust-dev is being deleted")
    );
    assert_eq!(draft(cx).disk_gb, "40");
    assert_eq!(question(cx), None);
    assert!(drawn(cx, "recipe-save"));
}

/// Closing Settings from another section must not ask about a draft the user cannot see: Settings shows Recipes again
/// behind the question, so the user knows which draft it is about. Specified: with a change typed and Hosts shown,
/// Escape shows Recipes again with the question up and focus on Save, and Settings stays open.
#[gpui::test]
fn a_close_from_another_section_shows_recipes_first(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    fill(cx, "recipe-disk", "30");
    click(cx, section_button(Section::Hosts));
    assert!(
        !drawn(cx, "recipe-editor"),
        "the premise: Recipes is hidden"
    );
    let _ = sent(&mut helm);
    keys(cx, "escape");
    assert!(any_open(cx));
    assert!(drawn(cx, "recipe-editor"), "Recipes is shown again");
    assert_eq!(question(cx).map(|(title, _)| title).as_deref(), Some(CLOSE));
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
}

// ---- Delete --------------------------------------------------------------------------------------------------------

/// Deleting a recipe cancels its builds and deletes its images and snapshots, so it asks first, in the old words, and
/// sends the delete only when confirmed, as the recipe's own work under its key. A new recipe has nothing to delete.
/// Specified: New recipe draws no Delete; on an Edit, Delete asks "Delete recipe rust-dev? …" with Cancel and Delete;
/// Cancel sends nothing; Delete again and confirmed sends `DELETE /api/tensorlake/recipes/rust-dev` under the recipe's
/// key, the form locked meanwhile; the done answer closes the editor with focus on New recipe.
#[gpui::test]
fn delete_asks_then_deletes_under_the_recipes_key(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, vec![entry("rust-dev", idle(), Value::Null)]);
    open_editor_with(&mut helm, cx, "recipe-new", json!([]));
    assert!(!drawn(cx, "recipe-delete"), "a new recipe has no Delete");
    click(cx, "recipe-back");
    open_editor_with(&mut helm, cx, "recipe-edit-rust-dev", json!([]));

    click(cx, "recipe-delete");
    assert_eq!(
        question(cx),
        Some((
            "Delete recipe rust-dev? Running builds are cancelled and Farhelm's images and snapshots for this \
             recipe are deleted. Images in use must be released first."
                .to_owned(),
            vec!["Cancel".into(), "Delete".into()]
        ))
    );
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-cancel"),
        "a risky action starts on Cancel"
    );
    // The footer question's long warning must wrap inside the dialog, where the whole consequence can be read.
    let prompt_bounds = bounds(cx, "prompt");
    let footer_bounds = bounds(cx, "settings-answers");
    assert!(prompt_bounds.left() >= footer_bounds.left());
    assert!(prompt_bounds.right() <= footer_bounds.right());
    assert!(
        !drawn(cx, "settings-footer-close"),
        "the question owns the footer"
    );
    click(cx, "prompt-cancel");
    assert_eq!(sent(&mut helm), [], "Cancel sends nothing");

    click(cx, "recipe-delete");
    click(cx, "prompt-confirm");
    let delete = one_command(&mut helm);
    assert_eq!(
        (delete.method, delete.path.as_str()),
        (Method::Delete, "/api/tensorlake/recipes/rust-dev")
    );
    assert_eq!(
        recipe_key(cx, "rust-dev").0,
        Some(Work::Save),
        "under the recipe's key"
    );
    // Asked of the drawn controls rather than by typing, which would move focus and so count as moving on.
    assert_eq!(
        available(cx, "recipe-base-Your image"),
        Some(false),
        "the form is locked"
    );
    assert_eq!(available(cx, "recipe-back"), Some(false));
    helm.reply(delete.id, 204, None);
    settle(cx);
    assert!(
        !drawn(cx, "recipe-editor"),
        "the done delete closed the editor"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("recipe-new"));
}

/// Only a second explicit answer lets the helm forget a capture whose image
/// is absent. Cancel is the default and leaves the recipe open without
/// resending Delete. Confirm uses the existing recipe operation key.
#[gpui::test]
fn an_unconfirmed_recipe_capture_requires_delete_anyway(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    for confirm in [false, true] {
        click(cx, "recipe-delete");
        click(cx, "prompt-confirm");
        let request = one_command(&mut helm);
        assert_eq!(
            (request.method, request.path.as_str()),
            (Method::Delete, "/api/tensorlake/recipes/rust-dev")
        );
        helm.reply_marked(
            request.id,
            409,
            vec![HelmMarker::RecipeCaptureConfirmation],
            Some(json!("the capture outcome is unknown")),
        );
        settle(cx);
        let (warning, answers) = question(cx).expect("missing capture asks in the footer");
        assert!(warning.contains("could still appear"), "{warning}");
        assert_eq!(answers, ["Cancel", "Delete anyway"]);
        assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));
        assert_eq!(recipe_key(cx, "rust-dev"), (None, None));
        if !confirm {
            click(cx, "prompt-cancel");
            assert!(drawn(cx, "recipe-editor"));
            assert_eq!(sent(&mut helm), []);
        }
    }
    click(cx, "prompt-confirm");
    let delete = one_command(&mut helm);
    assert_eq!(
        (delete.method, delete.path.as_str()),
        (
            Method::Delete,
            "/api/tensorlake/recipes/rust-dev?forget_missing_captures=true"
        )
    );
    assert_eq!(recipe_key(cx, "rust-dev").0, Some(Work::Save));
    helm.reply(delete.id, 204, None);
    settle(cx);
    assert!(!drawn(cx, "recipe-editor"));
}

/// The helm refuses a delete at once while the recipe is in use or already being deleted; the user needs its words
/// where they pressed Delete, and the row must not show them again once the editor closes, as for a refused save.
/// Specified: a 409 keeps the editor open with the helm's words and an editable form, and the recipe's key keeps no
/// refusal.
#[gpui::test]
fn a_refused_delete_stays_in_the_editor(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    click(cx, "recipe-delete");
    click(cx, "prompt-confirm");
    let delete = one_command(&mut helm);
    let reason = "recipe rust-dev is already being deleted";
    helm.reply(delete.id, 409, Some(json!(reason)));
    settle(cx);
    assert!(drawn(cx, "recipe-editor"));
    assert_eq!(label_of(cx, "recipe-editor-error").as_deref(), Some(reason));
    assert_eq!(
        recipe_key(cx, "rust-dev"),
        (None, None),
        "the refusal left the key"
    );
    fill(cx, "recipe-disk", "30");
    assert_eq!(draft(cx).disk_gb, "30", "editable again");
}

/// The helm answers a delete within 20 seconds; a cleanup still running then carries on, and the helm says so with a
/// 504. The old app had no bound, so this is this app's decision (see "Deviations" in the parity doc): the editor has
/// nothing left to edit, so it closes, and the helm's words stay on the recipe's key, where the row shows them until
/// the push takes the recipe away. Specified: a 504 closes the editor, and the row's line under its build shows the
/// helm's words.
#[gpui::test]
fn a_delete_that_carries_on_closes_the_editor_and_the_row_says_so(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    click(cx, "recipe-delete");
    click(cx, "prompt-confirm");
    let delete = one_command(&mut helm);
    let words = "recipe rust-dev's cleanup did not finish within 20 seconds; the deletion carries on, and the recipe \
                 leaves the recipe list once it is deleted; if it is still listed later, delete it again to see why";
    helm.reply(delete.id, 504, Some(json!(words)));
    settle(cx);
    assert!(!drawn(cx, "recipe-editor"), "the editor closed");
    assert_eq!(
        label_of(cx, "recipe-rust-dev-build-error").as_deref(),
        Some(words)
    );
}

/// The recipe's key admits one request at a time, and the row's build holds it while it is out; a confirmed Delete
/// that sends nothing must say why where it was pressed. Specified: with the recipe's build out, a confirmed Delete
/// sends nothing, the editor stays open and its error line reads "Not deleted: " and the operation store's shared
/// reason.
#[gpui::test]
fn a_delete_while_the_recipes_build_is_out_says_why_and_sends_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, vec![entry("rust-dev", idle(), Value::Null)]);
    click(cx, "recipe-build-rust-dev");
    let _build = one_command(&mut helm);
    // The editor's credential read waits behind the build, since the helm answers in order; Delete needs no answer.
    click(cx, "recipe-edit-rust-dev");
    assert_eq!(
        one_command(&mut helm).path,
        "/api/credentials",
        "the premise"
    );
    assert_eq!(
        recipe_key(cx, "rust-dev").0,
        Some(Work::Save),
        "the premise: the build is out on the recipe's key"
    );
    click(cx, "recipe-delete");
    click(cx, "prompt-confirm");
    assert_eq!(sent(&mut helm), [], "nothing more went out");
    assert!(drawn(cx, "recipe-editor"));
    assert_eq!(
        label_of(cx, "recipe-editor-error"),
        Some(format!(
            "Not deleted: {}",
            crate::operations::NotStarted::BUSY_REASON
        ))
    );
}

// ---- Closing while work is out -------------------------------------------------------------------------------------

/// A close while the save is out must neither drop the draft silently nor offer a Discard it cannot keep: the draft on
/// screen is the one being saved. The old editor asked here too and finished the close once the save was done, even
/// unanswered. Specified: with an update out, Escape asks with Keep editing and Save only and sends nothing more; the
/// question's Save sends nothing more and Settings stays open while the save is out; the done answer closes Settings.
/// Again with a second change: Escape asks, and the done answer closes Settings with the question unanswered.
#[gpui::test]
fn a_close_while_the_save_is_out_waits_for_it(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    fill(cx, "recipe-disk", "30");
    click(cx, "recipe-save");
    let save = one_command(&mut helm);
    leave_by(cx, Leave::Escape);
    assert_eq!(
        question(cx),
        Some((CLOSE.to_owned(), vec!["Keep editing".into(), "Save".into()])),
        "no Discard while the save is out"
    );
    click(cx, "prompt-confirm");
    assert_eq!(sent(&mut helm), [], "one save went out");
    assert!(any_open(cx), "Settings waits for the save");
    helm.reply(save.id, 200, Some(entry("rust-dev", idle(), Value::Null)));
    settle(cx);
    assert!(!any_open(cx), "the done save finished the close");

    open_settings(cx);
    click(cx, section_button(Section::Recipes));
    answer_account(&mut helm, cx, true);
    open_editor_with(&mut helm, cx, "recipe-edit-rust-dev", json!([]));
    fill(cx, "recipe-disk", "40");
    click(cx, "recipe-save");
    let save = one_command(&mut helm);
    leave_by(cx, Leave::Escape);
    assert_eq!(question(cx).map(|(title, _)| title).as_deref(), Some(CLOSE));
    helm.reply(save.id, 200, Some(entry("rust-dev", idle(), Value::Null)));
    settle(cx);
    assert!(
        !any_open(cx),
        "the done save finished the close it was asking about"
    );
}

/// A close while a delete is out has nothing to ask: the recipe is going, and the delete goes on in the operation
/// store as a save does. Specified: with a change typed and a confirmed delete out, Escape closes Settings at once with
/// no question.
#[gpui::test]
fn a_close_while_the_delete_is_out_closes_at_once(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    fill(cx, "recipe-disk", "30");
    click(cx, "recipe-delete");
    click(cx, "prompt-confirm");
    let delete = one_command(&mut helm);
    assert_eq!(delete.method, Method::Delete, "the premise");
    assert_eq!(draft(cx).disk_gb, "30", "the premise: unsaved changes");
    leave_by(cx, Leave::Escape);
    assert!(!any_open(cx), "Settings closed without asking");
}

/// A newer question owns the decision, and Keep editing means stay: an earlier answer's leave must not override what
/// the user answered since. Specified: Escape then the question's Save sends the update; while it is out, Escape asks
/// again, and Keep editing answers it; the done save then leaves Settings open with the list shown and the editor gone.
#[gpui::test]
fn keep_editing_after_an_earlier_save_answer_stays_in_settings(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    fill(cx, "recipe-disk", "30");
    leave_by(cx, Leave::Escape);
    click(cx, "prompt-confirm");
    let save = one_command(&mut helm);
    leave_by(cx, Leave::Escape);
    assert_eq!(
        question(cx).map(|(title, _)| title).as_deref(),
        Some(CLOSE),
        "asked again"
    );
    click(cx, "prompt-cancel");
    helm.reply(save.id, 200, Some(entry("rust-dev", idle(), Value::Null)));
    settle(cx);
    assert!(any_open(cx), "Keep editing kept Settings open");
    assert!(
        !drawn(cx, "recipe-editor") && drawn(cx, "recipes-list"),
        "the done save returned to the list"
    );
}

/// A question asked while the save was out offers no Discard; once that save is refused nothing is out, and keeping
/// such a question would leave the user only Save, which resends what the helm just refused. Specified: Escape while
/// the save is out asks; a 409 takes the question away, shows the helm's words and the ordinary answers, and focus
/// lands on Save.
#[gpui::test]
fn a_refused_save_takes_an_open_question_away(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    fill(cx, "recipe-disk", "30");
    click(cx, "recipe-save");
    let save = one_command(&mut helm);
    leave_by(cx, Leave::Escape);
    assert_eq!(
        question(cx).map(|(title, _)| title).as_deref(),
        Some(CLOSE),
        "the premise"
    );
    helm.reply(
        save.id,
        409,
        Some(json!("the recipe rust-dev is being deleted")),
    );
    settle(cx);
    assert_eq!(question(cx), None, "the question went");
    assert_eq!(
        label_of(cx, "recipe-editor-error").as_deref(),
        Some("the recipe rust-dev is being deleted")
    );
    assert!(drawn(cx, "recipe-save"));
    assert_eq!(focused_id(cx).as_deref(), Some("recipe-save"));
}

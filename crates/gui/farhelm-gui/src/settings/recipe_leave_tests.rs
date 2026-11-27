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
use super::tests::{
    available, drawn, fill, into_view, label_of, open_settings, painted, section_button, sent,
};
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

/// The Delete question's opening words for `rust-dev`, enough to tell whether it is drawn.
const DELETE_ASKED: &str = "Delete recipe rust-dev?";

/// The label of the focused control. Needed while an unsaved-changes question is up over an open Delete question: both
/// are drawn, and both use the shared prompt's answer ids, so an id cannot say which question holds focus.
fn focused_label(cx: &mut VisualTestContext) -> Option<String> {
    frame(cx)
        .into_iter()
        .find(|node| node.flags.focused && node.element_id.is_some())
        .and_then(|node| node.label.map(|label| label.to_string()))
}

/// Type a change into `rust-dev`'s editor and ask Delete, so its Delete question is up in the footer.
fn change_and_ask_delete(cx: &mut VisualTestContext) {
    fill(cx, "recipe-disk", "30");
    click(cx, "recipe-delete");
    assert!(
        painted(cx).contains(DELETE_ASKED),
        "the premise: the Delete question is up"
    );
}

/// Which unsaved-changes question a test asks over the Delete question.
#[derive(Clone, Copy, Debug)]
enum Over {
    /// The recipe editor's own question, asked by Back.
    Leave,
    /// Settings' question, asked by Escape from an editable field.
    Close,
}

impl Over {
    fn title(self) -> &'static str {
        match self {
            Over::Leave => LEAVE,
            Over::Close => CLOSE,
        }
    }
}

/// With the Delete question up, ask `over`'s question over it: the leave question by Back, or Settings' close question
/// by Escape from the disk field (the saved recipe's name is read-only and takes no focus). Back is the only way to the
/// leave question here: while the Delete question is up it replaces the footer row, the editor's own Cancel included.
/// Either question must open on its Save with the Delete question still drawn behind it; a test of what an answer does
/// to the Delete question needs that premise, or it would pass on an editor that took the question away on asking.
fn ask_over_delete(cx: &mut VisualTestContext, over: Over) {
    // Each control is wheeled into view first, as a user scrolls to it: typing into the disk field scrolls the body
    // down past Back, and scrolling back up for Back can leave the disk field below the fold, where a click misses.
    match over {
        Over::Leave => {
            into_view(cx, "recipe-back");
            click(cx, "recipe-back");
        }
        Over::Close => {
            into_view(cx, "recipe-disk");
            click(cx, "recipe-disk");
            keys(cx, "escape");
        }
    }
    let title = over.title();
    assert!(painted(cx).contains(title), "the premise: {title} is up");
    assert!(
        painted(cx).contains(DELETE_ASKED),
        "the premise: the Delete question stays behind {title}"
    );
    assert_eq!(
        focused_label(cx).as_deref(),
        Some("Save"),
        "the premise: {title} opens on its Save"
    );
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
/// The whole warning and both answers must fit at the minimum window size.
#[gpui::test]
fn delete_asks_then_deletes_under_the_recipes_key(cx: &mut TestAppContext) {
    let (mut helm, cx) = show(cx, vec![entry("rust-dev", idle(), Value::Null)]);
    open_editor_with(&mut helm, cx, "recipe-new", json!([]));
    assert!(!drawn(cx, "recipe-delete"), "a new recipe has no Delete");
    click(cx, "recipe-back");
    open_editor_with(&mut helm, cx, "recipe-edit-rust-dev", json!([]));

    cx.simulate_resize(crate::bootstrap::MIN_WINDOW_SIZE);
    settle(cx);
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
    let shown = frame(cx);
    let painted = shown
        .iter()
        .flat_map(|node| &node.texts)
        .filter(|line| line.visible_bounds.intersect(&footer_bounds) == line.bounds)
        .map(|line| line.text.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let warning = question(cx).unwrap().0;
    assert!(painted.contains(&warning), "visible footer text: {painted}");
    for id in ["prompt-cancel", "prompt-confirm"] {
        let answer = shown
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id));
        let answer = answer.unwrap_or_else(|| panic!("{id} was not drawn"));
        let visible = answer.visible_bounds.intersect(&footer_bounds);
        assert_eq!(visible, answer.bounds, "{id}");
    }
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

/// SPEC.md, "Opening and closing": nothing closes a confirmation it did not open, and Keep editing is the user taking
/// back the leave or the close, not answering the Delete. Before this was aligned with Settings' close and custom
/// agents, merely asking the recipe editor's leave question took the Delete question away, so Keep editing came back
/// to a footer without it. Specified: with a change typed and Delete asked, Back's leave question, and then Settings'
/// close question, each leave the Delete question drawn behind them; each one's Keep editing sends nothing and returns
/// to the editor with the Delete question still up; that question still answers, and its Delete sends
/// `DELETE /api/tensorlake/recipes/rust-dev`.
#[gpui::test]
fn keep_editing_keeps_an_unanswered_delete_question(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    change_and_ask_delete(cx);
    for over in [Over::Leave, Over::Close] {
        // Asserts, among its premises, that asking left the Delete question in the footer.
        ask_over_delete(cx, over);
        let title = over.title();
        // Keep editing is the first answer and Save the last, so two steps back from Save.
        keys(cx, "shift-tab shift-tab");
        assert_eq!(
            focused_label(cx).as_deref(),
            Some("Keep editing"),
            "{title}: the premise: Keep editing has focus"
        );
        keys(cx, "enter");
        assert!(
            !painted(cx).contains(title),
            "{title}: Keep editing took the question away"
        );
        assert!(
            any_open(cx) && drawn(cx, "recipe-editor"),
            "{title}: the editor stays"
        );
        assert!(
            painted(cx).contains(DELETE_ASKED),
            "{title}: the Delete question is still up"
        );
        assert_eq!(draft(cx).disk_gb, "30", "{title}: the draft is kept");
    }
    assert_eq!(sent(&mut helm), [], "Keep editing sends nothing");
    click(cx, "prompt-confirm");
    let delete = one_command(&mut helm);
    assert_eq!(
        (delete.method, delete.path.as_str()),
        (Method::Delete, "/api/tensorlake/recipes/rust-dev"),
        "the kept Delete question still answers"
    );
}

/// The leave question's Save is the newer answer, so it takes an unanswered Delete question away, as Settings' close
/// does (`a_close_save_takes_an_unanswered_delete_question_away`): a Delete still answerable after the user chose to
/// save could delete what they just kept. Specified: with a change typed and Delete asked, Back's leave question opens
/// with the Delete question still behind it; its Save sends the update, and the Delete question is gone while the save
/// is out.
#[gpui::test]
fn the_leave_questions_save_takes_an_unanswered_delete_question_away(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    change_and_ask_delete(cx);
    ask_over_delete(cx, Over::Leave);
    keys(cx, "enter");
    let update = one_command(&mut helm);
    assert_eq!(
        (update.method, update.path.as_str()),
        (Method::Put, "/api/tensorlake/recipes/rust-dev"),
        "the leave question's Save sent the update"
    );
    assert!(drawn(cx, "recipe-editor"), "the editor waits for the save");
    assert!(
        !painted(cx).contains(DELETE_ASKED),
        "the unanswered Delete question is gone"
    );
}

// ---- Closing while work is out -------------------------------------------------------------------------------------

/// An unanswered inline Delete confirmation must not outlive a newer answer: once the close question's Save sends the
/// recipe, a Delete still answerable in the footer could delete what the person just chose to keep (as a custom
/// agent's Save takes its own Delete question away, `custom_agents.rs` `Editor::save`). Specified: with a change typed
/// and the inline Delete asked, Escape from a field asks the close question with the Delete question still behind it;
/// its Save sends the update, and the Delete confirmation is gone.
#[gpui::test]
fn a_close_save_takes_an_unanswered_delete_question_away(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    change_and_ask_delete(cx);
    ask_over_delete(cx, Over::Close);
    keys(cx, "enter");
    let update = one_command(&mut helm);
    assert_eq!(update.method, Method::Put, "the close Save sent the update");
    assert!(
        !painted(cx).contains(DELETE_ASKED),
        "the unanswered Delete question is gone"
    );
}

/// SPEC.md, "Settings": waiting for the helm's round trip before Settings closes is accepted, so a close asked while
/// the recipe's save is out asks nothing about a draft already on its way, and waits for it. Specified: with an update
/// out, Escape asks nothing, sends nothing more and leaves Settings open; the done answer closes Settings.
#[gpui::test]
fn a_close_while_the_save_is_out_waits_for_it(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    fill(cx, "recipe-disk", "30");
    click(cx, "recipe-save");
    let save = one_command(&mut helm);
    leave_by(cx, Leave::Escape);
    assert_eq!(question(cx), None, "Escape waits for the save");
    assert_eq!(sent(&mut helm), [], "one save went out");
    assert!(any_open(cx), "Settings waits for the save");
    helm.reply(save.id, 200, Some(entry("rust-dev", idle(), Value::Null)));
    settle(cx);
    assert!(!any_open(cx), "the done save let the close go on");
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

/// The close question's Save already closes Settings when its saves are done, so a second Escape while that Save runs
/// must not ask again or close early over the save. Specified: Escape then the question's Save sends the update; while
/// it is out, Escape asks nothing and Settings stays open; the done save closes Settings.
#[gpui::test]
fn a_second_close_during_the_close_save_asks_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    fill(cx, "recipe-disk", "30");
    leave_by(cx, Leave::Escape);
    click(cx, "prompt-confirm");
    let save = one_command(&mut helm);
    leave_by(cx, Leave::Escape);
    assert_eq!(question(cx), None, "not asked again");
    assert!(any_open(cx));
    helm.reply(save.id, 200, Some(entry("rust-dev", idle(), Value::Null)));
    settle(cx);
    assert!(!any_open(cx), "the close Save finished the close");
}

/// A close that waited on a save the helm then refuses must not close over the refusal or ask to resend what the helm
/// just refused. Specified: Escape while the save is out asks nothing; a 409 keeps Settings open on the editor with the
/// helm's words and its ordinary Save, and still asks nothing.
#[gpui::test]
fn a_refused_save_keeps_a_waiting_close_open_on_the_editor(cx: &mut TestAppContext) {
    let (mut helm, cx) = edit_rust_dev(cx);
    fill(cx, "recipe-disk", "30");
    click(cx, "recipe-save");
    let save = one_command(&mut helm);
    leave_by(cx, Leave::Escape);
    assert_eq!(question(cx), None, "the premise: the close waits");
    helm.reply(
        save.id,
        409,
        Some(json!("the recipe rust-dev is being deleted")),
    );
    settle(cx);
    assert!(any_open(cx), "the refusal keeps Settings open");
    assert_eq!(question(cx), None, "and asks nothing");
    assert_eq!(
        label_of(cx, "recipe-editor-error").as_deref(),
        Some("the recipe rust-dev is being deleted")
    );
    assert!(drawn(cx, "recipe-save"));
}

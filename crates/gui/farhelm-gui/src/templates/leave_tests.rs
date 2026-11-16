//! Tests of leaving the template editor and of deleting templates: the leave question every departure asks about an
//! unsaved draft, its three answers, and Delete with its Undo notice, "Delete permanently" and their refusals.
//!
//! Driven through the Templates dialog over the helm client's fake, as the editor tests are, with their helpers. The
//! helm pushes a command's changes before its reply, so each delete or save that succeeds pushes the list first.

use std::time::Duration;

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::launcher::{LaunchTemplate, LauncherKind, TemplateFields};
use farhelm_proto::ui_channel::{Availability, CollectionChanges};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::dialog_tests::{open_header, painted, push, template, templates_state};
use super::editor::DeleteKind;
use super::editor_tests::{
    draft, drawn, name_focused, one, open_editor, push_preferences, renamed, templates,
};
use crate::layers;
use crate::test_support::{self, advance, any_open, click, frame, keys, settle};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The leave question's title.
const QUESTION: &str = "Save these changes before leaving?";

/// Open `codex-deep` and make its draft unsaved by picking another model suggestion.
fn open_unsaved(helm: &mut FakeHelm, cx: &mut VisualTestContext) {
    open_editor(helm, "template-open-codex-deep", cx);
    click(cx, "templates-suggestion-gpt-6-luna");
    assert!(
        painted(cx).contains("Unsaved changes"),
        "the premise: an unsaved draft"
    );
}

/// The helm deletes `name` and pushes the deletion, leaving `remaining`.
fn push_removal(
    helm: &mut FakeHelm,
    remaining: &[LaunchTemplate],
    name: &str,
    cx: &mut VisualTestContext,
) {
    let state = templates_state(remaining, Availability::Available);
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: Vec::new(),
        remove: vec![name.to_owned()],
    });
    settle(cx);
}

/// Press Delete on the open template `name` and let the helm delete it, leaving `remaining`.
fn delete_open(
    helm: &mut FakeHelm,
    name: &str,
    remaining: &[LaunchTemplate],
    cx: &mut VisualTestContext,
) {
    click(cx, "templates-delete");
    let (id, _) = one(helm, Method::Delete, &format!("/api/templates/{name}"));
    push_removal(helm, remaining, name, cx);
    helm.reply(id, 200, None);
    settle(cx);
}

/// `templates()` without `name`.
fn without(name: &str) -> Vec<LaunchTemplate> {
    templates()
        .into_iter()
        .filter(|template| template.name != name)
        .collect()
}

// ---- The leave question --------------------------------------------------------------------------------------------

/// SPEC.md: "Leaving an unsaved draft asks Keep editing, Discard, or Save in that reading order, with initial focus
/// still on Save". The question must stand in every way out the old dialog guarded, or an unsaved draft is lost by
/// the one that forgot it. Specified: a clean editor opens another row without asking; with an unsaved draft, another
/// row, "+ New template", Duplicate, the header Close and Escape each ask instead, with the answers in that order and
/// focus on Save, and the editor and dialog stay; Keep editing then puts focus back in the name field, sending nothing.
#[gpui::test]
fn every_departure_from_an_unsaved_draft_asks_first(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-webbuilder", cx);
    click(cx, "template-open-codex-deep");
    assert_eq!(draft(cx).name, "codex-deep", "a clean editor just leaves");
    click(cx, "templates-suggestion-gpt-6-luna");
    let departures: [fn(&mut VisualTestContext); 5] = [
        |cx| click(cx, "template-open-webbuilder"),
        |cx| click(cx, "templates-new"),
        |cx| click(cx, "templates-duplicate"),
        |cx| click(cx, "templates-close"),
        |cx| keys(cx, "escape"),
    ];
    for (index, depart) in departures.into_iter().enumerate() {
        depart(cx);
        settle(cx);
        assert!(painted(cx).contains(QUESTION), "departure {index} asks");
        let answers: Vec<String> = frame(cx)
            .into_iter()
            .filter_map(|node| node.element_id.map(|id| id.to_string()))
            .filter(|id| id.starts_with("prompt-") && id != "prompt")
            .collect();
        assert_eq!(
            answers,
            ["prompt-cancel", "prompt-alternative-0", "prompt-confirm"]
        );
        assert_eq!(
            test_support::focused_id(cx).as_deref(),
            Some("prompt-confirm")
        );
        assert!(any_open(cx) && draft(cx).name == "codex-deep");
        click(cx, "prompt-cancel");
        settle(cx);
        assert!(!painted(cx).contains(QUESTION));
        assert!(name_focused(cx), "Keep editing returns to the name field");
    }
    assert!(helm.commands().is_empty(), "nothing was sent");
}

/// Discard must carry out the departure the user chose, not a generic one. Specified: Discard on another row opens
/// that row's template clean; asked again and discarded for the header Close, the dialog closes.
#[gpui::test]
fn discard_goes_where_the_user_was_heading(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_unsaved(&mut helm, cx);
    click(cx, "template-open-webbuilder");
    click(cx, "prompt-alternative-0");
    settle(cx);
    assert_eq!(draft(cx).name, "webbuilder");
    assert!(!painted(cx).contains("Unsaved changes"));
    assert!(name_focused(cx));
    click(cx, "templates-new");
    cx.simulate_input("fresh");
    settle(cx);
    assert!(
        painted(cx).contains("Unsaved changes"),
        "the premise: the new draft is unsaved"
    );
    click(cx, "templates-close");
    click(cx, "prompt-alternative-0");
    settle(cx);
    assert!(!any_open(cx));
}

/// The old prompt's departure waited for its save, so a refused save never threw the draft away. Specified: the
/// question's Save sends the save and stays on the draft while it is in flight; once the helm stores it, the
/// departure (another row) goes. A refused save stays, with the helm's words in the status line and the draft kept.
#[gpui::test]
fn save_leaves_only_once_the_save_succeeds(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_unsaved(&mut helm, cx);
    click(cx, "template-open-webbuilder");
    click(cx, "prompt-confirm");
    let (id, _) = one(&mut helm, Method::Put, "/api/templates/codex-deep");
    helm.reply(id, 409, Some(json!("fixture save refused")));
    settle(cx);
    let text = painted(cx);
    assert!(text.contains("fixture save refused"), "{text}");
    assert_eq!(draft(cx).name, "codex-deep", "a refused save stays");
    assert!(text.contains("Unsaved changes"), "{text}");

    click(cx, "template-open-webbuilder");
    click(cx, "prompt-confirm");
    let (id, body) = one(&mut helm, Method::Put, "/api/templates/codex-deep");
    assert_eq!(draft(cx).name, "codex-deep", "it waits for the save");
    let mut stored = templates();
    stored[0].fields = serde_json::from_value(body).expect("the fields decode");
    push(&mut helm, &stored, Availability::Available, cx);
    helm.reply(id, 200, None);
    settle(cx);
    assert_eq!(draft(cx).name, "webbuilder");
    assert!(name_focused(cx));
}

/// SPEC.md's question is about where the user is heading now, so a second departure while it is up must not leave
/// the first one's answer behind. Specified: asked for another row, then for "+ New template", Discard opens a new
/// template.
#[gpui::test]
fn a_new_departure_replaces_the_question(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_unsaved(&mut helm, cx);
    click(cx, "template-open-webbuilder");
    click(cx, "templates-new");
    settle(cx);
    click(cx, "prompt-alternative-0");
    settle(cx);
    assert_eq!(draft(cx).name, "", "the new template, not webbuilder");
    assert!(painted(cx).contains("New template"));
}

// ---- Delete and Undo -----------------------------------------------------------------------------------------------

/// SPEC.md: "Delete takes effect immediately and offers undo inside the dialog for about ten seconds", and Undo
/// "never restores the mark" (or a key). Specified: Delete on the marked `codex-deep` with an unsaved edit sends the
/// deletion at once; once deleted, the editor goes, focus moves to "+ New template" and the notice says "Deleted
/// codex-deep."; Undo saves the stored copy (not the edit) under its old name and nothing else, and the notice goes.
#[gpui::test]
fn delete_takes_effect_at_once_and_undo_restores_the_loaded_copy(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), Some("codex-deep"));
    open_unsaved(&mut helm, cx);
    delete_open(&mut helm, "codex-deep", &without("codex-deep"), cx);
    assert!(
        painted(cx).contains("Choose a template to edit"),
        "the editor went"
    );
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("templates-new")
    );
    // The name is host text, drawn as a block of its own, so the painted lines part it from the period.
    assert!(
        painted(cx).contains("Deleted codex-deep"),
        "{}",
        painted(cx)
    );

    click(cx, "templates-undo");
    let (id, body) = one(&mut helm, Method::Put, "/api/templates/codex-deep");
    assert_eq!(
        body,
        json!({"kind": "agent", "agent": "codex", "model": "gpt-6.1-sol", "effort": "high"})
    );
    push(&mut helm, &templates(), Availability::Available, cx);
    helm.reply(id, 200, None);
    settle(cx);
    assert!(!drawn(cx, "templates-undo-notice"));
    assert!(helm.commands().is_empty(), "no mark or key was written");
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("templates-new"),
        "focus leaves the vanished Undo for + New template, as the old recovery put it"
    );
}

/// The notice's lifetime is SPEC.md's: about ten seconds, replaced by a later deletion, ended by closing the dialog.
/// A replaced notice must not be cut short by the earlier one's deadline. Specified: a second deletion six seconds
/// after the first shows its own notice, which lasts past the first one's ten seconds and is gone ten seconds after
/// it; closing the dialog with a notice up drops the dialog, its notice and its deadline with it, and the dialog
/// opened again shows none.
#[gpui::test]
fn undo_lasts_ten_seconds_a_later_delete_replaces_it_and_closing_ends_it(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    delete_open(&mut helm, "codex-deep", &without("codex-deep"), cx);
    advance(cx, Duration::from_secs(6));
    click(cx, "template-open-webbuilder");
    let remaining = vec![templates()[1].clone()];
    delete_open(&mut helm, "webbuilder", &remaining, cx);
    assert!(painted(cx).contains("Deleted webbuilder"));
    advance(cx, Duration::from_secs(6));
    assert!(drawn(cx, "templates-undo-notice"), "its own ten seconds");
    advance(cx, Duration::from_secs(4));
    assert!(!drawn(cx, "templates-undo-notice"));

    click(cx, "template-open-kindless");
    delete_open(&mut helm, "kindless", &[], cx);
    assert!(drawn(cx, "templates-undo-notice"), "the premise");
    let closed = cx.update(|_, cx| {
        layers::last_opened_weak::<super::dialog::TemplatesDialog>(cx).expect("a dialog was opened")
    });
    keys(cx, "escape");
    assert!(!any_open(cx));
    assert!(closed.upgrade().is_none(), "the closed dialog is gone");
    click(cx, "templates-button");
    assert!(!drawn(cx, "templates-undo-notice"));
}

/// SPEC.md: "undo refuses if its old name has been taken", while names stored with whitespace before the new-name
/// rule "retain delete undo". Specified: with `my review` (a spaced name storing workspace trust) deleted and then
/// created again by someone else, Undo is refused in the status line, sending nothing, and the notice stays; once the
/// name is free again, Undo saves it under the spaced name without workspace trust.
#[gpui::test]
fn undo_refuses_a_taken_name_and_keeps_a_spaced_one(cx: &mut TestAppContext) {
    let spaced = template(
        "my review",
        TemplateFields {
            kind: Some(LauncherKind::Command),
            command: Some("make review".into()),
            workspace_trust: Some(Some(true)),
            ..TemplateFields::default()
        },
    );
    let (mut helm, cx) = open_header(cx, std::slice::from_ref(&spaced), None);
    let path = ApiPath::api(&["templates", "my review"]).expect("a path");
    open_editor(&mut helm, "template-open-my review", cx);
    click(cx, "templates-delete");
    let (id, _) = one(&mut helm, Method::Delete, path.as_str());
    push_removal(&mut helm, &[], "my review", cx);
    helm.reply(id, 200, None);
    settle(cx);
    push(
        &mut helm,
        &[template("my review", TemplateFields::default())],
        Availability::Available,
        cx,
    );
    click(cx, "templates-undo");
    let text = painted(cx);
    assert!(
        text.contains("Another template is now named") && text.contains("cannot be restored"),
        "{text}"
    );
    assert!(helm.commands().is_empty());
    assert!(drawn(cx, "templates-undo-notice"));

    push_removal(&mut helm, &[], "my review", cx);
    click(cx, "templates-undo");
    let (_, body) = one(&mut helm, Method::Put, path.as_str());
    assert_eq!(body, json!({"kind": "command", "command": "make review"}));
}

/// SPEC.md: a stored template "that fails current save rules" shows "Delete permanently" and gets no undo, since Undo
/// saves through the ordinary path the helm would refuse. Specified: a template whose command holds a zero-width
/// space shows "Delete permanently"; deleting it shows no notice, and ends an earlier deletion's notice.
#[gpui::test]
fn a_template_failing_save_rules_is_deleted_permanently(cx: &mut TestAppContext) {
    let mut list = templates();
    list[2].fields.command = Some("make\u{200B}dev".into());
    let (mut helm, cx) = open_header(cx, &list, None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    let remaining: Vec<LaunchTemplate> = list[1..].to_vec();
    delete_open(&mut helm, "codex-deep", &remaining, cx);
    assert!(drawn(cx, "templates-undo-notice"), "the premise");
    click(cx, "template-open-webbuilder");
    assert!(painted(cx).contains("Delete permanently"));
    delete_open(&mut helm, "webbuilder", &list[1..2], cx);
    assert!(!drawn(cx, "templates-undo-notice"));
}

/// SPEC.md: "The label the user saw decides: a click on "Delete permanently" never offers undo, and a click on plain
/// "Delete" always does, even if the template list changed in between". The press hands the dialog the kind its
/// label was drawn with; this drives that hand-over directly with a kind the store would no longer decide. Specified:
/// the permanent kind on a template that passes the rules offers no Undo, and the ordinary kind on one that fails them
/// does.
#[gpui::test]
fn the_label_the_user_saw_decides_the_undo(cx: &mut TestAppContext) {
    let mut list = templates();
    list[2].fields.command = Some("make\u{200B}dev".into());
    let (mut helm, cx) = open_header(cx, &list, None);
    let cases = [
        ("codex-deep", DeleteKind::Permanent, false),
        ("webbuilder", DeleteKind::Restorable, true),
    ];
    for (round, (name, kind, undo)) in cases.into_iter().enumerate() {
        if round == 0 {
            open_editor(&mut helm, "template-open-codex-deep", cx);
        } else {
            click(cx, "template-open-webbuilder");
        }
        cx.update(|window, cx| {
            let dialog = layers::last_opened::<super::dialog::TemplatesDialog>(cx)
                .expect("a dialog is open");
            dialog.update(cx, |dialog, cx| dialog.delete_editor(kind, window, cx));
        });
        let (id, _) = one(&mut helm, Method::Delete, &format!("/api/templates/{name}"));
        helm.reply(id, 200, None);
        settle(cx);
        assert_eq!(drawn(cx, "templates-undo-notice"), undo, "{name}");
    }
}

/// A delete the helm refuses must say so and keep the template open, or the user believes it gone. Specified: the
/// helm's words show in the status line, the editor still holds the template, and no Undo is offered.
#[gpui::test]
fn a_refused_delete_is_reported_and_keeps_the_editor(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    click(cx, "templates-delete");
    let (id, _) = one(&mut helm, Method::Delete, "/api/templates/codex-deep");
    helm.reply(id, 409, Some(json!("fixture delete refused")));
    settle(cx);
    assert!(painted(cx).contains("fixture delete refused"));
    assert_eq!(draft(cx).name, "codex-deep");
    assert!(!drawn(cx, "templates-undo-notice"));
}

/// The editor keeps a template an agent deleted, and its Delete must not send a deletion with no copy to undo.
/// Specified: with `codex-deep` open and then deleted elsewhere, Delete sends nothing and says the template is no
/// longer stored.
#[gpui::test]
fn deleting_a_template_deleted_elsewhere_sends_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    push_removal(&mut helm, &without("codex-deep"), "codex-deep", cx);
    click(cx, "templates-delete");
    assert!(painted(cx).contains("This template is no longer stored"));
    assert!(helm.commands().is_empty());
}

/// The question sits at the editor's foot, which a short window leaves below the fold; the old prompt scrolled its
/// footer into view as it opened, and a question the user cannot see holds focus unseen. Specified: in a window too
/// short for the whole editor, Escape on an unsaved draft brings the question's Save inside the window.
#[gpui::test]
fn the_question_is_scrolled_into_view(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    cx.simulate_resize(gpui::size(gpui::px(1600.), gpui::px(800.)));
    settle(cx);
    open_unsaved(&mut helm, cx);
    assert!(
        test_support::bounds(cx, "templates-save").top() > gpui::px(800.),
        "the premise: the editor's foot is below the window"
    );
    keys(cx, "escape");
    settle(cx);
    let save = test_support::bounds(cx, "prompt-confirm");
    assert!(
        save.bottom() <= gpui::px(800.) && save.top() >= gpui::px(0.),
        "{save:?}"
    );
}

/// The old dialog had one status line, which every save cleared; an Undo refusal must not linger under the panes as if
/// it answered a later, unrelated save. Specified: with `codex-deep` deleted and its name taken again, Undo's refusal
/// shows while `webbuilder` is open; an edit and a successful save of `webbuilder` clear it.
#[gpui::test]
fn a_later_save_clears_an_undo_refusal(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    delete_open(&mut helm, "codex-deep", &without("codex-deep"), cx);
    click(cx, "template-open-webbuilder");
    push(&mut helm, &templates(), Availability::Available, cx);
    click(cx, "templates-undo");
    assert!(
        painted(cx).contains("cannot be restored"),
        "the premise: Undo was refused"
    );
    click(cx, "templates-command");
    cx.simulate_input("x");
    settle(cx);
    click(cx, "templates-save");
    let (id, _) = one(&mut helm, Method::Put, "/api/templates/webbuilder");
    helm.reply(id, 200, None);
    settle(cx);
    assert!(!painted(cx).contains("cannot be restored"));
}

/// A rename is four writes (the new name, the mark, the key, the old name's deletion), and leaving through the
/// question's Save must wait for the last: departing after the first would leave the rename half done under an editor
/// that no longer shows it. Specified: renaming the marked `codex-deep`, which holds key 1, then asking to open
/// `webbuilder` and answering Save, the editor still holds `codex-deeper` after each of the first three writes is
/// answered, and opens `webbuilder` once the old name's deletion is.
#[gpui::test]
fn save_from_the_question_waits_for_a_whole_rename(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), Some("codex-deep"));
    push_preferences(&mut helm, Some("codex-deep"), &[("1", "codex-deep")], cx);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    cx.simulate_input("er");
    settle(cx);
    click(cx, "template-open-webbuilder");
    click(cx, "prompt-confirm");
    let (id, _) = one(&mut helm, Method::Put, "/api/templates/codex-deeper");
    push(&mut helm, &renamed(true), Availability::Available, cx);
    helm.reply(id, 200, None);
    settle(cx);
    assert_eq!(draft(cx).name, "codex-deeper", "after the new name");
    let (id, _) = one(&mut helm, Method::Put, "/api/preferences");
    push_preferences(&mut helm, Some("codex-deeper"), &[("1", "codex-deep")], cx);
    helm.reply(id, 204, None);
    settle(cx);
    assert_eq!(draft(cx).name, "codex-deeper", "after the mark");
    let (id, _) = one(&mut helm, Method::Put, "/api/preferences");
    push_preferences(
        &mut helm,
        Some("codex-deeper"),
        &[("1", "codex-deeper")],
        cx,
    );
    helm.reply(id, 204, None);
    settle(cx);
    assert_eq!(draft(cx).name, "codex-deeper", "after the key");
    let (id, _) = one(&mut helm, Method::Delete, "/api/templates/codex-deep");
    push_removal(&mut helm, &renamed(false), "codex-deep", cx);
    helm.reply(id, 204, None);
    settle(cx);
    assert_eq!(draft(cx).name, "webbuilder");
}

/// The fields stay editable under the question, so the user may undo their edits before answering; the old prompt's
/// Save then left, having nothing to save. Specified: asked to open `webbuilder`, the model put back to its stored
/// value, Save opens `webbuilder` and sends nothing.
#[gpui::test]
fn save_on_a_draft_no_longer_unsaved_leaves_at_once(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_unsaved(&mut helm, cx);
    click(cx, "template-open-webbuilder");
    click(cx, "templates-suggestion-gpt-6.1-sol");
    assert!(
        !painted(cx).contains("Unsaved changes") && painted(cx).contains(QUESTION),
        "the premise: the edit is undone with the question up"
    );
    click(cx, "prompt-confirm");
    settle(cx);
    assert_eq!(draft(cx).name, "webbuilder");
    assert!(helm.commands().is_empty());
}

/// The question's answers stay pressable while a ⚡ write runs, and must then keep the draft rather than open another
/// template beside the write (a recorded deviation: the old answers were disabled). Specified: with the question up, a
/// bolt click's write unanswered, Discard closes the question and keeps the unsaved draft; once the write is answered,
/// asking again and discarding opens `webbuilder`.
#[gpui::test]
fn answers_while_a_bolt_write_runs_keep_the_draft(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_unsaved(&mut helm, cx);
    click(cx, "template-open-webbuilder");
    click(cx, "template-bolt-webbuilder");
    let (id, _) = one(&mut helm, Method::Put, "/api/preferences");
    click(cx, "prompt-alternative-0");
    settle(cx);
    assert!(!painted(cx).contains(QUESTION));
    assert_eq!(draft(cx).name, "codex-deep");
    assert!(painted(cx).contains("Unsaved changes"));
    helm.reply(id, 204, None);
    settle(cx);
    click(cx, "template-open-webbuilder");
    click(cx, "prompt-alternative-0");
    settle(cx);
    assert_eq!(draft(cx).name, "webbuilder");
}

/// The dialog's writes go one at a time, and a bolt click delivered in the same frame as a Delete press reaches a bolt
/// not yet drawn busy. Specified: with a deletion in flight, a bolt's toggle, as that click would run it, sends nothing.
#[gpui::test]
fn a_bolt_write_waits_for_a_delete_in_flight(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    click(cx, "templates-delete");
    one(&mut helm, Method::Delete, "/api/templates/codex-deep");
    cx.update(|_, cx| {
        let dialog =
            layers::last_opened::<super::dialog::TemplatesDialog>(cx).expect("a dialog is open");
        dialog.update(cx, |dialog, cx| dialog.toggle_mark("webbuilder", false, cx));
    });
    settle(cx);
    assert!(helm.commands().is_empty());
}

/// The mirror of the case above, the old spec's "deletion waits for an in-flight quick mark before clearing it": the
/// helm clears a deleted template's ⚡ mark with the deletion, so a deletion sent while a mark write is out could race
/// it, leaving a mark on a name that is gone or a cleared mark the user just set. Specified: with `codex-deep` open and
/// its bolt's write unanswered, Delete sends nothing and the editor stays on `codex-deep`; once the helm stores the
/// mark and answers, the early press is not replayed, and a new Delete deletes it and the notice says so.
#[gpui::test]
fn delete_waits_for_a_bolt_write_in_flight(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_header(cx, &templates(), None);
    open_editor(&mut helm, "template-open-codex-deep", cx);
    click(cx, "template-bolt-codex-deep");
    let (id, body) = one(&mut helm, Method::Put, "/api/preferences");
    assert_eq!(body, json!({"quick_launch_template": "codex-deep"}));
    click(cx, "templates-delete");
    assert!(
        helm.commands().is_empty(),
        "Delete waits for the bolt's write"
    );
    assert_eq!(draft(cx).name, "codex-deep", "and the editor stays");

    push_preferences(&mut helm, Some("codex-deep"), &[], cx);
    helm.reply(id, 204, None);
    settle(cx);
    assert!(
        helm.commands().is_empty(),
        "the early press was dropped, not replayed"
    );
    delete_open(&mut helm, "codex-deep", &without("codex-deep"), cx);
    assert!(
        painted(cx).contains("Deleted codex-deep"),
        "{}",
        painted(cx)
    );
}

/// The old "older template name deletion warns before permanently deleting": whether Delete can be undone depends on
/// the template as stored, which is what Undo would save back, never on the draft. A repaired draft that offered Undo
/// would promise a restore the helm refuses. Specified: a command template whose stored session name holds an emoji
/// joiner, its name field removed, added again and typed clean, still shows "Delete permanently", whose help says undo
/// is not available; deleting it sends the deletion and shows no Undo notice.
#[gpui::test]
fn a_repaired_draft_of_a_template_failing_save_rules_still_deletes_permanently(
    cx: &mut TestAppContext,
) {
    let mut list = templates();
    list[2].fields.name = Some("joined \u{1F469}\u{200D}\u{1F4BB}".into());
    let (mut helm, cx) = open_header(cx, &list, None);
    open_editor(&mut helm, "template-open-webbuilder", cx);
    click(cx, "templates-remove-name");
    click(cx, "templates-add");
    click(cx, "templates-add-name");
    click(cx, "templates-session-name");
    settle(cx);
    cx.simulate_input("repaired draft");
    settle(cx);
    assert_eq!(
        draft(cx).fields.name.as_deref(),
        Some("repaired draft"),
        "the premise: the draft's name is clean"
    );
    assert!(
        painted(cx).contains("Delete permanently"),
        "{}",
        painted(cx)
    );
    let help = crate::session_list::host_rail::tests::hover_help(cx, "templates-delete")
        .expect("the premise: Delete's help is showing");
    assert!(help.contains("undo is not available"), "{help}");
    let remaining: Vec<LaunchTemplate> = list[..2].to_vec();
    delete_open(&mut helm, "webbuilder", &remaining, cx);
    assert!(!drawn(cx, "templates-undo-notice"), "no Undo is offered");
}

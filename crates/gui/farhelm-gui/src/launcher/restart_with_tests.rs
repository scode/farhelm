//! Tests of Restart with in the real main view over the helm client's fake: when it is offered and why not, what its
//! main action says and sends, and how its reply settles, alike with plain Restart's; plus the YOLO question's guarded
//! answer, which Restart with offers only when it still changes the stored launch.
//!
//! The fleet is the entry tests' (`entry_tests::window_with`): session `a` runs on host9, launched as the helm
//! fixture's Codex launch (gpt-5, high effort, YOLO) with its conversation captured, so Restart with is offered.

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::SessionRow;
use farhelm_proto::{LaunchEffort, LaunchPermission, SessionInfo};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::draft::{LaunchDraft, Mode};
use super::entry::{open_restart_with, restart_with_unavailable};
use super::entry_tests::window_with;
use super::view_tests::{draft, name_of, shown_refusal};
use crate::layers;
use crate::local_state;
use crate::operations::Work;
use crate::session_list::outcome;
use crate::session_list::tests::{change, listing_meta, session};
use crate::test_support::{self, any_open, settle};

/// Open Restart with of `key`, as the header's menu handler would, and settle.
fn restart_with(cx: &mut VisualTestContext, key: &str) -> bool {
    let opened = cx.update(|window, cx| open_restart_with(key, window, cx));
    settle(cx);
    opened
}

/// The restarts (`POST /api/sessions/{id}/restart`) the window sent since the last call. Every other command is
/// answered at once; the catalog read fails, since an unread catalog refuses no stored choice.
fn restarts(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<FakeCommand> {
    cx.run_until_parked();
    let mut sent = Vec::new();
    for command in helm.commands() {
        if command.path.ends_with("/restart") {
            sent.push(command);
        } else if command.path.ends_with("launch-catalog") {
            helm.reply(
                command.id,
                503,
                Some(json!("catalog unavailable in this test")),
            );
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    settle(cx);
    sent
}

/// Change the open Restart with's effort to Medium, an edit, so its main action is not inactive.
fn edit_effort(cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        let launcher =
            layers::last_opened::<super::view::Launcher>(cx).expect("a launcher is open");
        launcher.update(cx, |launcher, cx| {
            launcher.draft.choose_effort(Some(LaunchEffort::Medium));
            cx.notify();
        });
    });
    settle(cx);
}

/// Press the main action and return the one restart it sent, checking that it sent exactly one and hid.
fn pressed(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> FakeCommand {
    assert!(
        restarts(helm, cx).is_empty(),
        "the premise: nothing sent yet"
    );
    test_support::click(cx, "launcher-launch");
    let sent = restarts(helm, cx);
    let [restart] = &sent[..] else {
        panic!(
            "one press sent one restart, not {} (the launcher says {:?})",
            sent.len(),
            name_of(cx, "launcher-refusal")
        );
    };
    assert!(!any_open(cx), "the launcher hid while its restart is out");
    restart.clone()
}

/// Make the helm list `a` idle, as a push while the launcher stays open.
fn a_idles(helm: &mut FakeHelm, cx: &mut VisualTestContext) {
    let idle = session("a").host(9).status(json!({"state": "idle"}));
    helm.push(change(
        listing_meta(&["a", "b", "c", "d"], 5, false),
        &[idle],
        &[],
    ));
    settle(cx);
}

/// The helm's fixture session with `edit` applied to its JSON, as a row.
fn row(edit: impl FnOnce(&mut Value)) -> SessionRow {
    let mut value = crate::test_support::session_entry("a").value;
    edit(&mut value);
    serde_json::from_value(value).expect("the fixture still reads as a row")
}

/// The header greys Restart with with the old app's reason whenever the session's launch cannot be restarted with
/// edits (SPEC.md, "Restart with": "visible but greyed out otherwise, with a hover tooltip and accessible description
/// explaining why"), so its reasons are pinned. Specified: a resumable agent launch has none; a command launch without a
/// resume command, a legacy launch and a harness without conversation reporting name theirs; an agent launch with a
/// stored resume command is offered even before its conversation is captured (the conversation field is then required).
#[test]
fn restart_with_names_why_it_is_not_offered() {
    use super::entry::restart_with_reason as reason;
    assert_eq!(reason(&row(|_| {})), None);
    assert_eq!(
        reason(&row(|value| {
            value["launch"] = json!({"kind": "command", "command": "make watch", "yolo": false, "agent": null,
                "resume": null});
            value["restart_offer"] = json!("no_resume_command");
        }))
        .as_deref(),
        Some("This command launch has no resume command, so it cannot be restarted; use Replace with")
    );
    assert_eq!(
        reason(&row(|value| {
            value["launch"] = json!({"kind": "legacy", "invocation": "bash", "agent_kind": "generic",
                "resume_template": null});
        }))
        .as_deref(),
        Some(
            "This session was created before launch kinds, so its launch cannot be changed; use Replace with"
        )
    );
    assert_eq!(
        reason(&row(|value| {
            value["restart_offer"] = json!("not_captured");
        })),
        None,
        "a stored resume command takes a named conversation"
    );
}

/// SPEC.md, "Restart with": the action reads "Stop and restart" while the shown agent is working, the launcher says it
/// stops the agent first, and a press is the consent to that stop; Restart with sends the edited launch through the
/// session's own restart route. Specified: with a running, Restart with of a after an effort edit marks the effort
/// "● Changed · was high" and nothing else, draws "Stop and restart" and the working warning, and Launch sends one
/// `POST /api/sessions/a/restart` whose `with` carries the Medium effort and whose `stop_if_running` is set; the pill
/// then says "Restarting on host9…" and a's row waits.
#[gpui::test]
fn restart_with_stops_a_working_agent_with_consent(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(restart_with(cx, "a"), "Restart with opened");
    assert!(matches!(draft(cx).mode, Mode::RestartWith(_)));
    assert_eq!(
        name_of(cx, "launcher-changed-effort"),
        None,
        "the premise: nothing edited yet"
    );
    edit_effort(cx);
    assert_eq!(
        name_of(cx, "launcher-changed-effort").as_deref(),
        Some("● Changed · was high")
    );
    assert_eq!(name_of(cx, "launcher-changed-permissions"), None);
    let action = name_of(cx, "launcher-launch").unwrap_or_default();
    assert!(action.starts_with("Stop and restart"), "{action}");
    assert_eq!(
        name_of(cx, "launcher-source-warning").as_deref(),
        Some("The agent is working. This action stops it before restarting.")
    );
    let restart = pressed(&mut helm, cx);
    assert_eq!(restart.path, "/api/sessions/a/restart");
    let body = restart.body.expect("a restart carries a body");
    assert_eq!(body["with"]["effort"], json!("medium"));
    assert_eq!(body["stop_if_running"], json!(true));
    test_support::advance(cx, std::time::Duration::from_millis(2_100));
    assert_eq!(
        name_of(cx, "launch-progress").as_deref(),
        Some("Restarting on host9…")
    );
    assert_eq!(
        cx.update(|_, cx| outcome::waited_seconds("a", Work::Restart, cx)),
        Some(2)
    );
}

/// An idle agent is stopped without asking, by the supervisor's own rule, so the action reads plain "Restart" and the
/// press gives no stop consent (the old `restart_needs_confirmation`). Specified: with a listed idle, the action reads
/// "Restart", no warning shows, and the restart's `stop_if_running` is false.
#[gpui::test]
fn restart_with_an_idle_agent_asks_no_consent(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(restart_with(cx, "a"));
    a_idles(&mut helm, cx);
    edit_effort(cx);
    let action = name_of(cx, "launcher-launch").unwrap_or_default();
    assert!(action.starts_with("Restart "), "{action}");
    assert_eq!(name_of(cx, "launcher-source-warning"), None);
    let body = pressed(&mut helm, cx).body.expect("a body");
    assert_eq!(body["stop_if_running"], json!(false));
}

/// "Restart with never navigates", and a refusal "leaves the dialog and its edits visible with the reason", led by
/// "Restart error." as the old launcher's, while the session's row keeps the same words as a refused plain Restart.
/// Specified: with b opened after the press, a success leaves b open (a navigating restart would have opened a) and the
/// launcher closed; then a refused Restart with brings the launcher back with "Restart error. <words>" and the Medium
/// effort, and the store's line for a is that Restart refusal.
#[gpui::test]
fn restart_with_settles_as_plain_restart_does(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(restart_with(cx, "a"));
    edit_effort(cx);
    let restart = pressed(&mut helm, cx);
    cx.update(|_, cx| local_state::select("b", cx));
    settle(cx);
    helm.reply(restart.id, 200, None);
    settle(cx);
    assert!(!any_open(cx), "a success closes the launcher for good");
    assert_eq!(
        cx.update(|_, cx| local_state::selected(cx)).as_deref(),
        Some("b")
    );
    cx.update(|_, cx| local_state::select("a", cx));
    settle(cx);

    assert!(restart_with(cx, "a"));
    edit_effort(cx);
    let restart = pressed(&mut helm, cx);
    helm.reply(restart.id, 409, Some(json!("a's host is unreachable")));
    settle(cx);
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Restart error. a's host is unreachable")
    );
    assert_eq!(draft(cx).agent.effort, Some(LaunchEffort::Medium));
    let line = cx.update(|_, cx| outcome::current_line("a", cx));
    let line = line.expect("the session's row has a line");
    assert_eq!(line.work, Work::Restart);
}

/// SPEC.md, "Creation": the YOLO question offers "Start guarded session instead" for Restart with "only when the result
/// still differs from the stored launch"; a guarded restart equal to the stored launch would have nothing to send (the
/// old `restart_guarded_changes`). Specified: Restart with of a guarded Codex launch switched to YOLO would return to
/// the stored launch on the guarded answer, so it is not offered; of the fixture's YOLO launch, guarded changes it.
#[test]
fn restart_with_offers_guarded_only_when_it_changes_the_launch() {
    let info = |permissions: &str| -> SessionInfo {
        let mut value = crate::test_support::session_entry("a").value;
        value["launch"]["selection"]["permissions"] = json!(permissions);
        serde_json::from_value(value).expect("a session")
    };
    let mut back_to_stored =
        LaunchDraft::restart_with("a", &info("guarded"), Some(9)).expect("offered");
    back_to_stored.choose_permissions(Some(LaunchPermission::Yolo));
    assert!(!back_to_stored.guarded_still_changes());
    back_to_stored.edit_conversation("named-thread".into());
    assert!(
        back_to_stored.guarded_still_changes(),
        "a named conversation is a change a guarded restart keeps"
    );
    let from_yolo = LaunchDraft::restart_with("a", &info("yolo"), Some(9)).expect("offered");
    assert!(from_yolo.guarded_still_changes());
    let new = LaunchDraft::new(Some(1), "~".into(), None);
    assert!(
        new.guarded_still_changes(),
        "every other mode starts something new"
    );
}

/// A host that asks before YOLO launches refuses the Restart with until the user answers, and that question is the
/// launcher's, not a failed Restart on the session's row (the old "with YOLO cancellation leaves no source failure"
/// cases). Specified: a YOLO refusal of a Restart with of a brings the launcher back asking, and the store holds no line
/// for a.
#[gpui::test]
fn the_yolo_question_of_a_restart_with_leaves_no_failure_on_the_session(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(restart_with(cx, "a"));
    edit_effort(cx);
    let restart = pressed(&mut helm, cx);
    helm.reply_marked(
        restart.id,
        409,
        vec![crate::operations::HelmMarker::YoloConfirmation],
        Some(json!("YOLO launches on host9 need --yolo")),
    );
    settle(cx);
    assert!(
        test_support::frame(cx)
            .iter()
            .any(|node| node.element_id.as_deref() == Some("prompt-cancel")),
        "the premise: the launcher asks the question"
    );
    assert_eq!(cx.update(|_, cx| outcome::current_line("a", cx)), None);
}

/// The command tab marks its edits too, with the old bare "● Changed", and Restart with of a resumable session is
/// offered (no greyed reason). Specified: `restart_with_unavailable` says nothing for e, a resumable command launch;
/// Restart with of e marks nothing until its command is edited, then marks the command and nothing else.
#[gpui::test]
fn restart_with_marks_an_edited_command(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("a"));
    assert_eq!(cx.update(|_, cx| restart_with_unavailable("e", cx)), None);
    assert!(restart_with(cx, "e"));
    assert_eq!(name_of(cx, "launcher-changed-command"), None, "the premise");
    cx.update(|_, cx| {
        let launcher =
            layers::last_opened::<super::view::Launcher>(cx).expect("a launcher is open");
        launcher.update(cx, |launcher, cx| {
            launcher
                .draft
                .edit_command("make watch --fast {farhelm_args}".into());
            cx.notify();
        });
    });
    settle(cx);
    assert_eq!(
        name_of(cx, "launcher-changed-command").as_deref(),
        Some("● Changed")
    );
    assert_eq!(name_of(cx, "launcher-changed-resume"), None);
    assert_eq!(name_of(cx, "launcher-changed-yolo"), None);
}

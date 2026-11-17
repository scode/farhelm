//! Tests of Restart with in the real main view over the helm client's fake: when it is offered and why not, what its
//! main action says and sends, how its reply settles, alike with plain Restart's, and what each answer to the YOLO
//! question sends; how it shows and compares a stored launch (escaped stored values, older or omitted permissions, the
//! changed markers); and a held Enter in its model field.
//!
//! The fleet is the entry tests' (`entry_tests::window_with`): session `a` runs on host9, launched as the helm
//! fixture's Codex launch (gpt-5, high effort, YOLO, and workspace trust on, which no restart may send back) with its
//! conversation captured, so Restart with is offered. `a_stores` replaces a's stored launch where a test needs another.

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::SessionRow;
use farhelm_proto::{LaunchEffort, LaunchPermission, SessionInfo};
use gpui::{Modifiers, TestAppContext, VisualTestContext, point, px};
use serde_json::{Value, json};

use super::draft::{LaunchDraft, Mode};
use super::entry::{open_restart_with, restart_with_unavailable};
use super::entry_tests::window_with;
use super::folder_default_tests::edit_draft;
use super::view_tests::{draft, launch_help, name_of, shown_refusal};
use super::yolo_tests::refuse_for_yolo;
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
    edit_draft(cx, |draft| draft.choose_effort(Some(LaunchEffort::Medium)));
}

/// A working session whose Restart with draft is ready to send. Move off the action after checking its ordinary
/// help so the test hovers afresh once the readiness changes. Answer background requests before the test starts
/// holding an operation, since the fake helm replies in order.
fn ready_to_restart(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(restart_with(cx, "a"));
    edit_effort(cx);
    assert!(restarts(&mut helm, cx).is_empty());
    assert_eq!(
        launch_help(cx).as_deref(),
        Some(
            "Stop and restart: stop the working agent, then relaunch this session with these settings"
        ),
        "the premise: the edited launch is ready"
    );
    cx.simulate_mouse_move(point(px(0.), px(0.)), None, Modifiers::none());
    assert!(restarts(&mut helm, cx).is_empty(), "hover sends no restart");
    (helm, cx)
}

/// Check the reason while `path` is unanswered. The window may keep reading in the background, and the fake
/// cannot answer a read queued behind the held operation. Observe the commands and leave their replies pending.
fn hover_while_held(helm: &mut FakeHelm, cx: &mut VisualTestContext, path: &str, reason: &str) {
    let mut sent = helm.commands();
    assert!(
        sent.iter().any(|command| command.path == path),
        "the premise: {path} is out, saw {sent:?}"
    );
    assert_eq!(launch_help(cx).as_deref(), Some(reason));
    sent.extend(helm.commands());
    assert!(
        !sent
            .iter()
            .any(|command| command.path.ends_with("/restart")),
        "hover sends no restart: {sent:?}"
    );
}

/// A draft based on old launch settings needs to be reopened. Hovering after the helm pushes different source
/// settings tells the user to close and reopen Restart with before they press Restart.
#[gpui::test]
fn restart_with_hover_explains_a_changed_stored_launch(cx: &mut TestAppContext) {
    let (mut helm, cx) = ready_to_restart(cx);
    // Keep the source working and on its original install so the stored effort is the only launch change.
    let a = session("a").host(9).tabs(1);
    let mut changes = change(listing_meta(&["a", "b", "c", "d"], 5, false), &[a], &[]);
    let row = &mut changes.upsert[0].value;
    row["host_identity"] = json!("identity9");
    assert_eq!(row["launch"]["selection"]["effort"], json!("high"));
    row["launch"]["selection"]["effort"] = json!("low");
    helm.push(changes);
    settle(cx);
    assert_eq!(
        launch_help(cx).as_deref(),
        Some("This session's launch settings changed; close and reopen restart with")
    );
    assert!(restarts(&mut helm, cx).is_empty(), "hover sends no restart");
}

/// Restart with shares the source session's operation key. While a Stop of that session is unanswered, hover help
/// tells the user to wait and sends no competing request.
#[gpui::test]
fn restart_with_hover_explains_its_sources_other_operation(cx: &mut TestAppContext) {
    let (mut helm, cx) = ready_to_restart(cx);
    cx.update(|_, cx| {
        crate::session_list::stop::send(
            "a",
            farhelm_proto::SessionStopRequest {
                refuse_if_tabs_open: false,
                refuse_if_agent_running: false,
            },
            cx,
        );
    });
    settle(cx);
    hover_while_held(
        &mut helm,
        cx,
        "/api/sessions/a/stop",
        "Wait for this session's other operation to finish",
    );
}

/// Host-registry changes exclude Restart with as well as New. Hover help tells the user to wait for an unanswered
/// removal even when it targets a different host, since the window excludes launches during a registry change.
#[gpui::test]
fn restart_with_hover_explains_a_host_registry_change(cx: &mut TestAppContext) {
    let (mut helm, cx) = ready_to_restart(cx);
    let path = super::launch_tests::start_host_change(cx, super::launch_tests::HostChange::Remove);
    hover_while_held(&mut helm, cx, path, "Wait for the host change to finish");
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
/// `POST /api/sessions/a/restart` whose `with` carries the Medium effort and not the stored workspace trust (SPEC.md,
/// "Creation": the old remembered trust value is neither read nor written; restart-with.spec's "restart with choice
/// Enter applies effort and permissions without old trust") and whose `stop_if_running` is set; the pill then says
/// "Restarting on host9…" and a's row waits.
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
    assert!(
        body["with"]["workspace_trust"].is_null(),
        "the stored trust is no choice to send: {body}"
    );
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
/// Restart with of e marks nothing until its command is edited, then marks the command and nothing else; an edited
/// resume command then marks the resume command too, and a changed YOLO answer the answer; Restart sends all three
/// edits (restart-with.spec's "restart with edits a command launch's command, resume command and YOLO answer").
#[gpui::test]
fn restart_with_marks_an_edited_command(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert_eq!(cx.update(|_, cx| restart_with_unavailable("e", cx)), None);
    assert!(restart_with(cx, "e"));
    assert_eq!(name_of(cx, "launcher-changed-command"), None, "the premise");
    edit_draft(cx, |draft| {
        draft.edit_command("make watch --fast {farhelm_args}".into())
    });
    assert_eq!(
        name_of(cx, "launcher-changed-command").as_deref(),
        Some("● Changed")
    );
    assert_eq!(name_of(cx, "launcher-changed-resume"), None);
    assert_eq!(name_of(cx, "launcher-changed-yolo"), None);
    // The resume command and the YOLO answer are marked as they are edited, each on its own.
    edit_draft(cx, |draft| {
        draft.edit_command_resume("make watch --fast --resume {conversation} {farhelm_args}".into())
    });
    assert_eq!(
        name_of(cx, "launcher-changed-resume").as_deref(),
        Some("● Changed")
    );
    assert_eq!(name_of(cx, "launcher-changed-yolo"), None);
    edit_draft(cx, |draft| draft.choose_command_yolo(true));
    assert_eq!(
        name_of(cx, "launcher-changed-yolo").as_deref(),
        Some("● Changed")
    );
    let body = pressed(&mut helm, cx).body.expect("a body");
    let sent = &body["with_command"];
    assert_eq!(
        (&sent["command"], &sent["resume"], &sent["yolo"]),
        (
            &json!("make watch --fast {farhelm_args}"),
            &json!("make watch --fast --resume {conversation} {farhelm_args}"),
            &json!(true)
        ),
        "the three edits are sent whole: {body}"
    );
}

// ---- Stored command values shown escaped ---------------------------------------------------------------------------

/// `e`'s stored command and resume command for the escaped-display tests: an invisible character in each, a doubled
/// space and trailing spaces (the bytes most easily lost to a trim), which must all reach the helm exactly as stored.
const RAW_COMMAND: &str = "make\u{200b}  watch {farhelm_args}  ";
const RAW_RESUME: &str = "make\u{202e} watch --resume {conversation} {farhelm_args}  ";

/// The helm lists `e` idle (so Restart with asks no consent) with [`RAW_COMMAND`] and [`RAW_RESUME`] as its stored
/// command launch, keeping its recorded install.
fn e_stores_invisible_characters(helm: &mut FakeHelm, cx: &mut VisualTestContext) {
    let e = session("e")
        .host(9)
        .title("fix login")
        .unmanaged()
        .not_launched_as_checkout()
        .status(json!({"state": "idle"}))
        .launch(json!({
            "kind": "command",
            "command": RAW_COMMAND,
            "yolo": false,
            "agent": "codex",
            "resume": RAW_RESUME,
        }));
    let mut changes = change(listing_meta(&["a", "b", "c", "d"], 5, false), &[e], &[]);
    changes.upsert[0].value["host_identity"] = json!("identity9");
    helm.push(changes);
    settle(cx);
}

/// Restart with shows a stored command line and resume command holding invisible characters escaped and read-only,
/// each with its own Clear (the old launcher's Clear agent command and Clear resume command), so a stored command that
/// came from elsewhere cannot hide what it runs. Specified: both fields show their `<U+…>` spellings and refuse typing
/// until their Clear empties them; the stored command typed back in is the stored command again, so it is shown
/// spelled out and read-only with its Clear (typed text is shown as typed until then, `view::shown_text`).
#[gpui::test]
fn restart_with_shows_stored_command_values_escaped(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    e_stores_invisible_characters(&mut helm, cx);
    assert!(restart_with(cx, "e"));
    super::view_tests::escaped_until_cleared(
        cx,
        "launcher-command-row",
        "launcher-clear-command",
        "make<U+200B>  watch {farhelm_args}  ",
        |launcher| &launcher.command,
    );
    super::view_tests::escaped_until_cleared(
        cx,
        "launcher-resume-command-row",
        "launcher-clear-resume",
        "make<U+202E> watch --resume {conversation} {farhelm_args}  ",
        |launcher| &launcher.resume,
    );
    test_support::click(cx, "launcher-command-row");
    test_support::keys(cx, "backspace");
    cx.simulate_input(RAW_COMMAND);
    settle(cx);
    assert_eq!(
        cx.update(|_, cx| {
            let launcher =
                layers::last_opened::<super::view::Launcher>(cx).expect("a launcher is open");
            launcher.read(cx).command.text(cx).to_string()
        }),
        "make<U+200B>  watch {farhelm_args}  ",
        "typed back in, the stored command is spelled out again"
    );
    assert!(
        cx.debug_bounds("launcher-clear-command").is_some(),
        "with its Clear"
    );
}

/// What is shown escaped is still sent as stored (restart-with.spec's "restart with preserves raw command bytes for a
/// YOLO-only edit"): the escaped spelling never reaches the draft. Specified: with only the YOLO assertion changed, the
/// restart carries the command and resume command byte for byte as stored.
#[gpui::test]
fn restart_with_sends_stored_command_bytes_for_a_yolo_only_edit(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    e_stores_invisible_characters(&mut helm, cx);
    assert!(restart_with(cx, "e"));
    test_support::click(cx, "launcher-yolo-Yes (YOLO)");
    let body = pressed(&mut helm, cx)
        .body
        .expect("a restart carries a body");
    assert_eq!(
        body["with_command"]["yolo"],
        json!(true),
        "the premise: the edit went out"
    );
    assert_eq!(
        body["with_command"]["command"],
        json!(RAW_COMMAND),
        "{body}"
    );
    assert_eq!(body["with_command"]["resume"], json!(RAW_RESUME), "{body}");
}

/// The other half of restart-with.spec's raw-bytes case ("beside an edited command"): replacing the command leaves the
/// untouched resume command as stored. Specified: Clear agent command, a new command typed, Restart pressed: the
/// restart carries the new command and the stored resume command byte for byte.
#[gpui::test]
fn restart_with_sends_the_stored_resume_beside_an_edited_command(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    e_stores_invisible_characters(&mut helm, cx);
    assert!(restart_with(cx, "e"));
    test_support::click(cx, "launcher-clear-command");
    test_support::click(cx, "launcher-command-row");
    test_support::keys(cx, "r u n space { f a r h e l m _ a r g s }");
    let body = pressed(&mut helm, cx)
        .body
        .expect("a restart carries a body");
    assert_eq!(
        body["with_command"]["command"],
        json!("run {farhelm_args}"),
        "{body}"
    );
    assert_eq!(body["with_command"]["resume"], json!(RAW_RESUME), "{body}");
}

// ---- Stored agent launches, refusals and the YOLO question's answers ------------------------------------------------

/// The helm lists `a` idle (so Restart with asks no consent) as an agent launch of `selection` with a stored resume
/// command, keeping its recorded install, so Restart with edits that selection.
fn a_stores(helm: &mut FakeHelm, cx: &mut VisualTestContext, selection: Value) {
    let a = session("a")
        .host(9)
        .status(json!({"state": "idle"}))
        .launch(json!({
            "kind": "agent",
            "selection": selection,
            "start": ["agent", "{farhelm_args}"],
            "resume": ["agent", "resume", "{conversation}", "{farhelm_args}"],
        }));
    let mut changes = change(listing_meta(&["a", "b", "c", "d"], 5, false), &[a], &[]);
    changes.upsert[0].value["host_identity"] = json!("identity9");
    helm.push(changes);
    settle(cx);
}

/// Whether a fresh frame paints `shown`, a host value's escaped spelling, inside its own direction isolate within the
/// element `id`, as every host value in a sentence is drawn (`host_text`), so its hidden characters cannot reorder the
/// words around it. Scoped to the element because the same host words can be painted elsewhere in the window: a refused
/// restart's words are also on the session's row and in its header.
fn paints_isolated_in(cx: &mut VisualTestContext, id: &str, shown: &str) -> bool {
    crate::session_list::host_rail::tests::texts_within(&test_support::frame(cx), id)
        .iter()
        .any(|text| crate::host_text::tests::paints_isolated(&text.text, shown))
}

/// The one restart `helm` received since the last look, with no host write beside it.
fn one_restart_and_no_host_write(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Value {
    cx.run_until_parked();
    let commands = helm.commands();
    assert!(
        !commands
            .iter()
            .any(|command| command.path.ends_with("yolo-without-asking")),
        "no answer here writes the host setting"
    );
    let restarts: Vec<_> = commands
        .iter()
        .filter(|command| command.path.ends_with("/restart"))
        .collect();
    let [restart] = &restarts[..] else {
        panic!("one restart, not {}", restarts.len());
    };
    restart.body.clone().expect("a restart carries a body")
}

/// An older launch stored no permission, which for Codex is the agent's own setting, so choosing Guarded is an edit:
/// marked with what it was, and sent (restart-with.spec's "restart with upgrades an older own-setting launch to
/// guarded"). Specified: a stores Codex, gpt-6-astra, high, no permission; Guarded marks "● Changed · was agent's own
/// setting", and the restart sends that model and effort with Guarded.
#[gpui::test]
fn guarded_over_an_older_own_setting_launch_is_an_edit(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    a_stores(
        &mut helm,
        cx,
        json!({"harness": "codex", "model": "gpt-6-astra", "effort": "high", "permissions": null}),
    );
    assert!(restart_with(cx, "a"));
    assert_eq!(
        draft(cx).agent.permissions,
        None,
        "the premise: own setting"
    );
    test_support::click(cx, "launcher-permissions-Guarded");
    assert_eq!(
        name_of(cx, "launcher-changed-permissions").as_deref(),
        Some("● Changed · was agent's own setting")
    );
    let body = pressed(&mut helm, cx).body.expect("a body");
    assert_eq!(
        (
            &body["with"]["model"],
            &body["with"]["effort"],
            &body["with"]["permissions"]
        ),
        (&json!("gpt-6-astra"), &json!("high"), &json!("guarded"))
    );
}

/// An omitted permission is compared by the mode it ran with, not as a mode of its own: an OMP launch stored with none
/// ran YOLO, so YOLO chosen again after another mode is no edit (restart-with.spec's "restart with compares an older
/// omitted permission by its effective mode"). Specified: a stores OMP with nothing chosen; it shows YOLO; Approve
/// marks the permission; YOLO again takes the marker away, and a press sends nothing, refused as unchanged.
#[gpui::test]
fn an_omitted_permission_is_compared_by_the_mode_it_ran_with(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    a_stores(
        &mut helm,
        cx,
        json!({"harness": "omp", "model": null, "effort": null, "permissions": null}),
    );
    assert!(restart_with(cx, "a"));
    assert_eq!(
        draft(cx).agent.permissions,
        Some(LaunchPermission::Yolo),
        "the premise: shown as the YOLO it ran with"
    );
    test_support::click(cx, "launcher-permissions-Approve");
    assert!(name_of(cx, "launcher-changed-permissions").is_some());
    test_support::click(cx, "launcher-permissions-YOLO");
    assert_eq!(name_of(cx, "launcher-changed-permissions"), None);
    restarts(&mut helm, cx);
    test_support::click(cx, "launcher-launch");
    assert!(
        restarts(&mut helm, cx).is_empty(),
        "an unchanged launch has nothing to restart with"
    );
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Restart error. Change a setting before restarting"),
        "refused as unchanged"
    );
}

/// The old model in a changed marker is the host's text, so a hidden character in it is spelled out
/// (restart-with.spec's "restart with escapes the old model in its changed marker"). Specified: a stores Codex with the
/// model `gpt\u{202e}5x`; choosing the default model marks "● Changed · was gpt<U+202E>5x", the old model painted
/// inside its own direction isolate.
#[gpui::test]
fn the_changed_marker_spells_out_the_old_models_hidden_characters(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    a_stores(
        &mut helm,
        cx,
        json!({"harness": "codex", "model": "gpt\u{202e}5x", "effort": null, "permissions": "guarded"}),
    );
    assert!(restart_with(cx, "a"));
    edit_draft(cx, LaunchDraft::choose_default_model);
    assert_eq!(
        name_of(cx, "launcher-changed-model").as_deref(),
        Some("● Changed · was gpt<U+202E>5x")
    );
    assert!(
        paints_isolated_in(cx, "launcher-changed-model", "gpt<U+202E>5x"),
        "the old model is drawn in its own isolate"
    );
}

/// The helm's refusal can quote a peer's hidden characters, which Restart with spells out beside its action, the
/// launcher staying open with the edit (restart-with.spec's "Restart with escapes and isolates peer refusal text").
/// Specified: a refusal "refused \u{202e}host\u{200b}" reads "Restart error. refused <U+202E>host<U+200B>", the helm's
/// words painted inside their own direction isolate.
#[gpui::test]
fn a_restart_refusal_spells_out_hidden_characters(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(restart_with(cx, "a"));
    edit_effort(cx);
    let restart = pressed(&mut helm, cx);
    helm.reply(restart.id, 400, Some(json!("refused \u{202e}host\u{200b}")));
    settle(cx);
    assert!(any_open(cx), "the launcher came back");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Restart error. refused <U+202E>host<U+200B>")
    );
    assert!(
        paints_isolated_in(cx, "launcher-refusal", "refused <U+202E>host<U+200B>"),
        "the helm's words are drawn in their own isolate"
    );
}

/// Restart with asks the YOLO question as New does, and its one-time answer sends the same restart again with consent
/// (restart-with.spec's "a YOLO restart-with requires confirmation inside the dialog"). Specified: after a YOLO refusal
/// of an edited Restart with of a, "Start YOLO session anyway" sends one restart with the same `with` and stop consent,
/// and `confirm_yolo` set.
#[gpui::test]
fn the_one_time_yolo_answer_resends_the_restart(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(restart_with(cx, "a"));
    edit_effort(cx);
    let first = pressed(&mut helm, cx);
    refuse_for_yolo(&mut helm, cx, first.id);
    test_support::click(cx, "prompt-confirm");
    let retry = one_restart_and_no_host_write(&mut helm, cx);
    let first = first.body.expect("a body");
    assert_eq!(retry["confirm_yolo"], json!(true));
    assert_eq!(retry["with"], first["with"]);
    assert_eq!(retry["stop_if_running"], first["stop_if_running"]);
}

/// With YOLO shown, the question's guarded answer restarts with the edit made guarded, without consent or a host write
/// (restart-with.spec's "restart with can answer the YOLO question by restarting guarded"). Specified: Medium effort
/// chosen on a, whose stored launch is YOLO, refused for YOLO: "Start guarded session instead" sends one restart with
/// Medium and Guarded and no `confirm_yolo`.
#[gpui::test]
fn the_guarded_answer_restarts_guarded(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(restart_with(cx, "a"));
    edit_effort(cx);
    let first = pressed(&mut helm, cx);
    refuse_for_yolo(&mut helm, cx, first.id);
    assert_eq!(
        name_of(cx, "prompt-alternative-0").as_deref(),
        Some("Start guarded session instead")
    );
    test_support::click(cx, "prompt-alternative-0");
    let retry = one_restart_and_no_host_write(&mut helm, cx);
    assert_eq!(retry["confirm_yolo"], json!(false));
    assert_eq!(
        (&retry["with"]["effort"], &retry["with"]["permissions"]),
        (&json!("medium"), &json!("guarded"))
    );
}

/// A command restart asserted YOLO gets the question worded for an assertion, and its one-time answer resends the
/// edited command launch whole (restart-with.spec's "a YOLO command restart-with asks inside the dialog, worded for an
/// assertion"). Specified: e's YOLO answer changed to yes, refused for YOLO: the question says the command was asserted
/// to bypass its safety checks, and "Start YOLO session anyway" sends one restart with the same `with_command` and
/// `confirm_yolo`.
#[gpui::test]
fn a_command_restarts_yolo_question_speaks_of_the_assertion(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    e_stores_invisible_characters(&mut helm, cx);
    assert!(restart_with(cx, "e"));
    test_support::click(cx, "launcher-yolo-Yes (YOLO)");
    let first = pressed(&mut helm, cx);
    assert_eq!(
        first.body.as_ref().expect("a body")["with_command"]["yolo"],
        json!(true),
        "the premise: the restart asserted YOLO"
    );
    refuse_for_yolo(&mut helm, cx, first.id);
    assert_eq!(
        name_of(cx, "yolo-reason").as_deref(),
        Some("This command was asserted to bypass its safety checks.")
    );
    test_support::click(cx, "prompt-confirm");
    let retry = one_restart_and_no_host_write(&mut helm, cx);
    assert_eq!(retry["confirm_yolo"], json!(true));
    assert_eq!(
        retry["with_command"],
        first.body.expect("a body")["with_command"]
    );
}

// ---- The catalog -------------------------------------------------------------------------------------------------

/// The catalog's Retry reads the catalog again without seeding the draft again, so an edit made while the read had
/// failed is kept and sent (restart-with.spec's "restart with retries the catalog without reseeding its draft").
/// Specified: the first read fails and says so; Guarded is chosen; Retry's read answers with a's model; the failure
/// line goes, Guarded stays, and the restart sends it with the stored model and effort.
#[gpui::test]
fn the_catalog_retry_keeps_an_edit(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(restart_with(cx, "a"));
    super::view_tests::answer_catalog(&mut helm, cx, None);
    assert!(
        name_of(cx, "launcher-catalog-error").is_some(),
        "the premise: the read failed"
    );
    test_support::click(cx, "launcher-permissions-Guarded");
    test_support::click(cx, "launcher-catalog-retry");
    super::view_tests::answer_catalog(
        &mut helm,
        cx,
        Some(json!([{ "id": "gpt-5", "harness": "codex", "efforts": ["high", "low"] }])),
    );
    assert_eq!(name_of(cx, "launcher-catalog-error"), None);
    assert_eq!(draft(cx).agent.permissions, Some(LaunchPermission::Guarded));
    let body = pressed(&mut helm, cx).body.expect("a body");
    assert_eq!(
        (
            &body["with"]["model"],
            &body["with"]["effort"],
            &body["with"]["permissions"]
        ),
        (&json!("gpt-5"), &json!("high"), &json!("guarded"))
    );
}

// ---- A timed-out restart ----------------------------------------------------------------------------------------

/// A Restart with whose reply is the host's timeout may still restart the session: its row says so once the reply
/// comes, even with the launcher out of sight, and the launcher, brought back from its notice, shows the helm's words
/// rather than a refusal's, with its edit (nonblocking-launch.spec's "Restart with settles its unknown before its
/// launcher can return"). Specified: a key press after Restart, then a 504 with the bounded route's words: no launcher
/// opens, and the store's line for a is a Restart with those words; Open launcher brings it back showing them alone,
/// with the Medium effort.
#[gpui::test]
fn a_timed_out_restart_with_settles_its_row_before_the_launcher_returns(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(restart_with(cx, "a"));
    edit_effort(cx);
    let restart = pressed(&mut helm, cx);
    super::launch_tests::times_out_after_moving_on(&mut helm, cx, restart.id, "a", Work::Restart);
    assert_eq!(
        draft(cx).agent.effort,
        Some(LaunchEffort::Medium),
        "with its edit"
    );
}

// ---- Held keys ---------------------------------------------------------------------------------------------------

/// A held Enter in the model field applies nothing, as the old field's ignored an auto-repeat, so a key still down from
/// an earlier control cannot choose a model on its own (restart-with.spec's "restart with model Enter refuses an
/// unselected draft and held keys", its held half). Specified: `other` typed into Restart with's model field, with the
/// catalog read failed so typed text applies as a custom id; Enter delivered as an auto-repeat leaves the stored model
/// and the launcher open; a fresh Enter then applies `other`.
#[gpui::test]
fn a_held_enter_in_the_model_field_does_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(restart_with(cx, "a"));
    restarts(&mut helm, cx);
    test_support::click(cx, "launcher-model-row");
    cx.simulate_input("other");
    settle(cx);
    assert_eq!(
        super::view_tests::read(cx, |launcher, _, cx| launcher
            .model_input
            .text(cx)
            .to_string()),
        "other",
        "the premise: the model field has the typed text"
    );
    cx.simulate_event(gpui::KeyDownEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    settle(cx);
    assert!(any_open(cx));
    assert_eq!(
        draft(cx).agent.model.as_deref(),
        Some("gpt-5"),
        "a held Enter applied nothing"
    );
    test_support::keys(cx, "enter");
    assert_eq!(
        draft(cx).agent.model.as_deref(),
        Some("other"),
        "a fresh Enter applies the typed id"
    );
}

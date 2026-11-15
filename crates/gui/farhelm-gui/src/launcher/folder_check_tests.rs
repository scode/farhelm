//! Tests of the launcher's folder check in the real main view over the helm client's fake: when it asks, what it sends,
//! what it says under the folder field, and which answers it ignores; plus the same marking for a refused launch.
//!
//! The launcher opens on New on host1 (connected) in `/nope`, a Command tab draft so that a press can launch. The fake
//! answers only when told, so each test holds the check's answer until it has set up the moment it arrives in.

use std::time::Duration;

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::draft::{LaunchDraft, Tab};
use super::view_tests::{Opened, answer_catalog, launcher_on, name_of};
use crate::test_support::{self, settle};

/// New on host1 in `folder`, on the Command tab with a complete command.
fn draft_in(folder: &str) -> LaunchDraft {
    let mut draft = LaunchDraft::new(Some(1), folder.into(), None);
    draft.choose_tab(Tab::Command);
    draft.edit_command("true".into());
    draft.choose_command_yolo(false);
    draft
}

/// The folder checks (`POST /api/browse-directory`) the window sent since the last call (the fake hands each command
/// out once).
fn checks(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<FakeCommand> {
    cx.run_until_parked();
    helm.commands()
        .into_iter()
        .filter(|command| command.path == "/api/browse-directory")
        .collect()
}

/// The line under the folder field, if any.
fn problem(cx: &mut VisualTestContext) -> Option<String> {
    name_of(cx, "launcher-folder-problem")
}

/// Type `text` as the folder, as an edit of the draft.
fn edit_folder(cx: &mut VisualTestContext, text: &str) {
    cx.update(|_, cx| {
        let launcher = cx
            .try_global::<Opened>()
            .and_then(|opened| opened.0.upgrade())
            .expect("a launcher is open");
        launcher.update(cx, |launcher, cx| {
            launcher.draft.edit_folder(text.to_owned());
            cx.notify();
        });
    });
    settle(cx);
}

/// SPEC.md, "Folder check": the launcher checks the folder "150 ms after the host or path stops changing", and "a
/// missing folder or a path that is not a directory marks the field and says so underneath, naming the host", and "the
/// warning is advisory and never disables Launch". Specified: nothing is asked within 100 ms of opening; by 160 ms one
/// membership-only check of `/nope` on host1 went out; its "does not exist" refusal puts "⚠ This folder does not exist
/// on host1." under the field; Launch still sends a create; refused for another reason, the launcher comes back still
/// marked, and editing the folder then takes the line away at once.
#[gpui::test]
fn a_missing_folder_is_said_under_the_field(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, draft_in("/nope"));
    answer_catalog(&mut helm, cx, Some(json!([])));
    test_support::advance(cx, Duration::from_millis(100));
    assert!(checks(&mut helm, cx).is_empty(), "not before 150 ms");
    test_support::advance(cx, Duration::from_millis(60));
    let sent = checks(&mut helm, cx);
    let [check] = &sent[..] else {
        panic!("one check, not {}", sent.len());
    };
    let body = check.body.clone().expect("a check carries a body");
    assert_eq!(body["host"], json!(1));
    assert_eq!(body["cwd"], json!("/nope"));
    assert_eq!(body["membership_only"], json!(true));
    assert_eq!(
        problem(cx),
        None,
        "the premise: nothing said before the answer"
    );
    helm.reply(
        check.id,
        400,
        Some(json!("working directory does not exist: /nope")),
    );
    settle(cx);
    assert_eq!(
        problem(cx).as_deref(),
        Some("⚠ This folder does not exist on host1.")
    );
    test_support::click(cx, "launcher-launch");
    cx.run_until_parked();
    let creates: Vec<FakeCommand> = helm
        .commands()
        .into_iter()
        .filter(|command| command.path == "/api/sessions")
        .collect();
    let [create] = &creates[..] else {
        panic!(
            "the warning did not stop Launch, so one create, not {}",
            creates.len()
        );
    };
    // Refused for another reason, the launcher comes back on the same folder, still marked.
    helm.reply(create.id, 409, Some(json!("host1 is busy")));
    settle(cx);
    assert_eq!(
        problem(cx).as_deref(),
        Some("⚠ This folder does not exist on host1."),
        "the premise: back on the marked folder"
    );
    edit_folder(cx, "/tmp");
    assert_eq!(
        problem(cx),
        None,
        "the line goes with the folder it was about"
    );
}

/// "An answer for an earlier host, connection or path is discarded", and checks stay one at a time, as the old
/// launcher's did (its "a stale check is dropped and checks remain serial while a reply is held"): a slow host must not
/// collect a read per pause in typing. Specified: with the check of `/nope` out, editing the folder to `/tmp` and
/// waiting past 150 ms sends nothing more; the held answer ("does not exist") then says nothing, since it is about
/// `/nope`, and the check of `/tmp` goes out at once; its usable answer says nothing either.
#[gpui::test]
fn checks_are_serial_and_a_stale_answer_is_discarded(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, draft_in("/nope"));
    answer_catalog(&mut helm, cx, Some(json!([])));
    test_support::advance(cx, Duration::from_millis(160));
    let first = checks(&mut helm, cx);
    assert_eq!(first.len(), 1, "the premise: the first check is out");
    edit_folder(cx, "/tmp");
    test_support::advance(cx, Duration::from_millis(300));
    assert!(
        checks(&mut helm, cx).is_empty(),
        "no second check while the first is out"
    );
    helm.reply(
        first[0].id,
        400,
        Some(json!("working directory does not exist: /nope")),
    );
    settle(cx);
    assert_eq!(problem(cx), None, "the stale answer says nothing");
    let second = checks(&mut helm, cx);
    let [check] = &second[..] else {
        panic!(
            "one new check once the first answered, not {}",
            second.len()
        );
    };
    assert_eq!(
        check.body.as_ref().map(|body| body["cwd"].clone()),
        Some(json!("/tmp"))
    );
    helm.reply(
        check.id,
        200,
        Some(json!({"cwd": "/tmp", "parent": "/", "children": [], "truncated": false})),
    );
    settle(cx);
    assert_eq!(problem(cx), None);
}

/// "Permission errors, timeouts, busy hosts and other inconclusive answers show no warning": a folder that may well
/// exist must not be marked. The supervisor reports a permission error as "not usable", which is one of its folder
/// phrasings, so the check must treat it as inconclusive (the old case "an inconclusive folder check shows no warning"
/// used that very answer). Specified: a "not usable ... (permission denied)" answer says nothing under the field, and
/// neither does a 504 timeout to the check that follows an edit.
#[gpui::test]
fn an_inconclusive_answer_says_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, draft_in("/slow"));
    answer_catalog(&mut helm, cx, Some(json!([])));
    test_support::advance(cx, Duration::from_millis(160));
    let sent = checks(&mut helm, cx);
    assert_eq!(sent.len(), 1, "the premise: the check is out");
    helm.reply(
        sent[0].id,
        400,
        Some(json!(
            "working directory is not usable: /slow (permission denied)"
        )),
    );
    settle(cx);
    assert_eq!(problem(cx), None, "a permission error is inconclusive");
    edit_folder(cx, "/slower");
    test_support::advance(cx, Duration::from_millis(160));
    let sent = checks(&mut helm, cx);
    assert_eq!(sent.len(), 1, "the premise: the next check is out");
    helm.reply(sent[0].id, 504, Some(json!("the host timed out")));
    settle(cx);
    assert_eq!(problem(cx), None, "a timeout is inconclusive");
}

/// SPEC.md, "Creation": "a folder refusal also marks the folder field naming the host", whatever the check said before
/// (the folder can go between the check and the press). Specified: with the check answered usable (nothing said), a
/// create the helm refuses as "is not a directory" brings the launcher back with the line naming host1.
#[gpui::test]
fn a_folder_refusal_marks_the_field(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, draft_in("/etc/hosts"));
    answer_catalog(&mut helm, cx, Some(json!([])));
    test_support::advance(cx, Duration::from_millis(160));
    let sent = checks(&mut helm, cx);
    assert_eq!(sent.len(), 1, "the premise: the check is out");
    helm.reply(
        sent[0].id,
        200,
        Some(json!({"cwd": "/etc/hosts", "parent": "/etc", "children": [], "truncated": false})),
    );
    settle(cx);
    assert_eq!(problem(cx), None, "the premise: the check found it usable");
    test_support::click(cx, "launcher-launch");
    cx.run_until_parked();
    let creates: Vec<FakeCommand> = helm
        .commands()
        .into_iter()
        .filter(|command| command.path == "/api/sessions")
        .collect();
    let [create] = &creates[..] else {
        panic!("one create, not {}", creates.len());
    };
    helm.reply(
        create.id,
        409,
        Some(json!(
            "create refused: working directory is not a directory: /etc/hosts"
        )),
    );
    settle(cx);
    assert_eq!(
        problem(cx).as_deref(),
        Some("⚠ This path is not a folder on host1.")
    );
}

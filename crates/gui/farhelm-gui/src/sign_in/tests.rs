//! Drive sign-in through the real window and fake pushed collections so focus and wire behavior share the same test.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::provisioning::{ProvisioningOperation, RunStatus};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, HostStateView, HostView, KeyedEntry,
    Preferences, SshConnectionPurpose, SshQuestion,
};
use gpui::{TestAppContext, VisualTestContext, px, size};
use serde_json::json;

use crate::hosts::tests::{aliased, connected, connecting, host, hosts, progress, run};
use crate::session_list::host_rail::tests::answer_everything;
use crate::session_list::tests::{listing, preferences_state, session};
use crate::test_support::{
    MainWindow, any_open, click, focused_id, frame, keys, label, meta, open_main, settle,
    terminal_focused,
};

const FIRST: &str = "question-one";
const SECOND: &str = "question-two";
const FIELD: &str = "ssh-sign-in-answer";
const ANSWER: &str = "host-attention-sign-in-2";

/// GPUI Kit gives every input the same inner id. Walk the focused node's ancestry to find this form's field wrapper.
fn answer_focused(cx: &mut VisualTestContext) -> bool {
    let nodes = frame(cx);
    nodes.iter().filter(|node| node.flags.focused).any(|node| {
        let mut at = Some(node);
        while let Some(node) = at {
            if node.element_id.as_deref() == Some(FIELD) {
                return true;
            }
            at = node.parent.map(|parent| &nodes[parent]);
        }
        false
    })
}

pub(crate) fn remote(id: i64, alias: &str, state: HostStateView) -> HostView {
    aliased(
        host(id, "ssh", state),
        alias,
        &format!("dev@{alias}.example.invalid"),
    )
}

pub(crate) fn question(id: &str, host: i64, prompt: &str, notice: bool) -> SshQuestion {
    SshQuestion {
        id: id.to_owned(),
        attempt: id.to_owned(),
        target: farhelm_proto::ui_channel::SshQuestionTarget::Host { host },
        purpose: SshConnectionPurpose::Sessions,
        startup: false,
        prompt: prompt.to_owned(),
        notice,
        expires_at: i64::MAX,
    }
}

/// Name the first-dial premise explicitly. A question from any later attempt uses the ordinary fixture above.
pub(crate) fn startup_question(id: &str, host: i64, prompt: &str, notice: bool) -> SshQuestion {
    SshQuestion {
        startup: true,
        ..question(id, host, prompt, notice)
    }
}

/// The full collection names each question by its reply id, just as the production helm does.
pub(crate) fn questions(questions: &[SshQuestion]) -> CollectionState {
    let order = questions
        .iter()
        .map(|question| question.id.as_str())
        .collect::<Vec<_>>();
    CollectionState {
        collection: Collection::SshQuestions,
        meta: meta(&order),
        entries: questions
            .iter()
            .map(|question| KeyedEntry {
                key: question.id.clone(),
                value: serde_json::to_value(question).unwrap(),
            })
            .collect(),
    }
}

fn publish(
    helm: &mut FakeHelm,
    pending: &[SshQuestion],
    gone: &[&str],
    cx: &mut VisualTestContext,
) {
    let state = questions(pending);
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: gone.iter().map(|id| (*id).to_owned()).collect(),
    });
    settle(cx);
}

/// Keep one ordinary local session alive so the tests can observe input sent to an actual terminal widget.
fn open<'a>(
    cx: &'a mut TestAppContext,
    pending: &[SshQuestion],
    remote_state: HostStateView,
) -> MainWindow<'a> {
    let mut window = open_main(
        cx,
        vec![
            listing(&["local"], &[session("local").host(1)]),
            preferences_state(Preferences {
                welcome_seen: Some(true),
                visit_history: vec!["local".to_owned()],
                ..Preferences::default()
            }),
            hosts(&[
                host(1, "local", connected()),
                remote(2, "build-box", remote_state),
                remote(3, "bench-box", connected()),
            ]),
            questions(pending),
            progress(&[]),
        ],
    );
    window.cx.simulate_resize(size(px(1000.), px(700.)));
    settle(window.cx);
    answer_everything(&mut window.helm);
    settle(window.cx);
    window
}

/// The new host's setup master can ask and finish before the sessions master has any question. The accepted
/// setup answer carries the Add action to that distinct master while the user is still waiting for it.
#[gpui::test]
fn add_run_can_focus_the_sessions_question_after_setup_finishes(cx: &mut TestAppContext) {
    let mut app = open(cx, &[], connecting(1, None));
    keys(app.cx, "escape");
    let mut run = run(2, ProvisioningOperation::Add, RunStatus::Running);
    let progress_state = |view| {
        let state = progress(&[view]);
        CollectionChanges {
            collection: state.collection,
            meta: state.meta,
            upsert: state.entries,
            remove: Vec::new(),
        }
    };
    app.helm.push(progress_state(run.clone()));
    settle(app.cx);
    app.cx.update(|window, cx| {
        let mark = crate::focus::Mark::take(cx);
        super::started_run(2, "run-2".to_owned(), true, mark, window, cx);
    });
    let mut setup = question(FIRST, 2, "Setup verification code:", false);
    setup.purpose = SshConnectionPurpose::Setup;
    publish(&mut app.helm, &[setup], &[], app.cx);
    assert!(answer_focused(app.cx), "the setup question opened first");
    keys(app.cx, "7 enter");
    let command = app
        .helm
        .commands()
        .into_iter()
        .find(|cmd| cmd.path == format!("/api/ssh-questions/{FIRST}"))
        .expect("the setup answer was sent");
    publish(&mut app.helm, &[], &[FIRST], app.cx);
    run.status = RunStatus::Completed;
    app.helm.push(progress_state(run));
    app.helm.reply(command.id, 204, None);
    settle(app.cx);
    assert!(!any_open(app.cx), "the sessions question has yet to arrive");

    publish(
        &mut app.helm,
        &[question(SECOND, 2, "Sessions verification code:", false)],
        &[],
        app.cx,
    );
    assert!(
        answer_focused(app.cx),
        "the separate master opened on its answer field"
    );
}

/// A startup question opens on its answer field before any key or click. Enter sends the exact question id, then
/// an accepted reply hands focus to another host that was waiting when the user sent it.
#[gpui::test]
fn startup_opens_the_question_and_send_can_focus_the_next_one(cx: &mut TestAppContext) {
    let first = startup_question(FIRST, 2, "Verification code: ", false);
    let second = question(SECOND, 3, "Confirm user presence for key", true);
    let mut app = open(cx, &[first, second.clone()], connected());
    assert!(answer_focused(app.cx));
    assert_eq!(label(app.cx, ANSWER).as_deref(), Some("Answer"));

    keys(app.cx, "1 2 3 enter");
    let command = app
        .helm
        .commands()
        .into_iter()
        .find(|cmd| cmd.path.contains("/ssh-questions/"))
        .unwrap();
    assert_eq!(command.path, format!("/api/ssh-questions/{FIRST}"));
    assert_eq!(
        command.body,
        Some(json!({ "action": "send", "answer": "123" }))
    );

    publish(&mut app.helm, &[second], &[FIRST], app.cx);
    assert!(
        !any_open(app.cx),
        "the collection retired the answered question before the reply"
    );
    app.helm.reply(command.id, 204, None);
    settle(app.cx);
    assert_eq!(focused_id(app.cx).as_deref(), Some("ssh-sign-in-cancel"));
    assert!(
        frame(app.cx)
            .iter()
            .all(|node| node.element_id.as_deref() != Some(FIELD)),
        "a touch notice has no input"
    );
    assert!(
        label(app.cx, "ssh-sign-in-send").is_none(),
        "a touch notice has no Send"
    );
    publish(&mut app.helm, &[], &[SECOND], app.cx);
    assert!(!any_open(app.cx), "the helper's exit removes the notice");
}

/// A physical touch is outside Farhelm. The same SSH process can follow it with a prompt, but a later reconnect
/// cannot borrow that focus opportunity after the answer was sent and the process finished.
#[gpui::test]
fn a_touch_and_answer_only_hand_focus_to_the_same_attempt_or_already_waiting_hosts(
    cx: &mut TestAppContext,
) {
    let touch = question(FIRST, 2, "Confirm user presence for key", true);
    let mut followup = question(SECOND, 2, "Verification code:", false);
    followup.attempt = touch.attempt.clone();
    let mut app = open(cx, &[touch], connected());
    assert!(
        !any_open(app.cx),
        "premise: the reconnect notice waits in the sidebar"
    );
    click(app.cx, ANSWER);
    assert_eq!(focused_id(app.cx).as_deref(), Some("ssh-sign-in-cancel"));

    publish(&mut app.helm, &[followup], &[FIRST], app.cx);
    assert!(answer_focused(app.cx));
    keys(app.cx, "9 enter");
    let command = app
        .helm
        .commands()
        .into_iter()
        .find(|cmd| cmd.path.contains("/ssh-questions/"))
        .unwrap();
    assert_eq!(command.path, format!("/api/ssh-questions/{SECOND}"));
    publish(&mut app.helm, &[], &[SECOND], app.cx);
    app.helm.reply(command.id, 204, None);
    settle(app.cx);
    assert!(!any_open(app.cx));

    publish(
        &mut app.helm,
        &[question("later-drop", 2, "Password:", false)],
        &[],
        app.cx,
    );
    assert_eq!(label(app.cx, ANSWER).as_deref(), Some("Answer"));
    assert!(
        !any_open(app.cx),
        "a later reconnect needs fresh intent even on the same host"
    );
}

/// A pending action can be laid out below the visible attention region. The automatic popover must wait until it
/// has scrolled that control into view. The clipped row is never an acceptable placement.
#[gpui::test]
fn startup_reveals_the_waiting_row_before_anchoring_the_popup(cx: &mut TestAppContext) {
    use crate::session_list::host_rail::tests::unreachable;
    use crate::test_support::bounds;

    const ACTION: &str = "host-attention-sign-in-17";
    let mut fleet = vec![host(1, "local", connected())];
    fleet.extend((2..17).map(|id| remote(id, "offline", unreachable())));
    fleet.push(remote(17, "sign-in-box", HostStateView::SignInNeeded));
    let mut app = open_main(
        cx,
        vec![
            listing(&[], &[]),
            preferences_state(Preferences {
                welcome_seen: Some(true),
                ..Preferences::default()
            }),
            hosts(&fleet),
            questions(&[]),
        ],
    );
    app.cx.simulate_resize(size(px(1000.), px(600.)));
    settle(app.cx);
    answer_everything(&mut app.helm);
    settle(app.cx);
    let in_view = |cx: &mut VisualTestContext| {
        let body = bounds(cx, "host-attention-body");
        frame(cx)
            .iter()
            .any(|node| node.element_id.as_deref() == Some(ACTION) && body.intersects(&node.bounds))
    };
    assert!(
        !in_view(app.cx),
        "the premise: the sign-in action starts below the fold"
    );

    publish(
        &mut app.helm,
        &[startup_question(FIRST, 17, "Password:", false)],
        &[],
        app.cx,
    );
    assert!(in_view(app.cx));
    assert!(answer_focused(app.cx));
}

/// Typing in a live terminal ends the startup focus opportunity. A later prompt leaves Enter and ordinary keys
/// with the program, then an explicit Answer opens the field and Cancel sends no answer.
#[gpui::test]
fn an_ssh_question_mid_work_waits_without_taking_terminal_input(cx: &mut TestAppContext) {
    let mut app = open(cx, &[], connected());
    app.terminal_live();
    keys(app.cx, "x");
    assert_eq!(
        app.program_input(),
        1,
        "premise: the program accepted the first key"
    );
    publish(
        &mut app.helm,
        &[startup_question(FIRST, 2, "Verification code:", false)],
        &[],
        app.cx,
    );
    assert_eq!(label(app.cx, ANSWER).as_deref(), Some("Answer"));
    assert!(!any_open(app.cx));
    assert!(terminal_focused(app.cx));
    keys(app.cx, "a enter");
    assert_eq!(app.program_input(), 2);

    click(app.cx, ANSWER);
    assert!(answer_focused(app.cx));
    keys(app.cx, "m n p");
    let sees_answer = |cx: &mut VisualTestContext| {
        frame(cx)
            .into_iter()
            .flat_map(|node| node.texts)
            .any(|text| text.text.contains("mnp"))
    };
    assert!(!sees_answer(app.cx), "the answer starts masked");
    click(app.cx, "ssh-sign-in-visibility");
    assert_eq!(
        label(app.cx, "ssh-sign-in-visibility").as_deref(),
        Some("Hide")
    );
    assert!(sees_answer(app.cx), "Show displays the same answer");
    click(app.cx, "ssh-sign-in-visibility");
    assert!(!sees_answer(app.cx), "Hide masks the answer again");
    click(app.cx, "ssh-sign-in-cancel");
    let command = app
        .helm
        .commands()
        .into_iter()
        .find(|cmd| cmd.path.contains("/ssh-questions/"))
        .unwrap();
    assert_eq!(command.path, format!("/api/ssh-questions/{FIRST}"));
    assert_eq!(command.body, Some(json!({ "action": "cancel" })));
}

/// A window left untouched since launch can still receive a later retry or reconnect question. The next key stays
/// with the selected terminal even though the sidebar now has a live Answer action.
#[gpui::test]
fn a_later_question_keeps_focus_in_an_untouched_window(cx: &mut TestAppContext) {
    let mut app = open(cx, &[], connected());
    app.terminal_live();
    assert!(terminal_focused(app.cx), "premise: the terminal has focus");
    publish(
        &mut app.helm,
        &[question(FIRST, 2, "Verification code:", false)],
        &[],
        app.cx,
    );
    assert_eq!(label(app.cx, ANSWER).as_deref(), Some("Answer"));
    assert!(!any_open(app.cx));
    assert!(terminal_focused(app.cx));
    keys(app.cx, "x");
    assert_eq!(app.program_input(), 1);
}

/// An explicit Sign in renews the focus opportunity only for the host the user started. Its action uses the normal
/// retry route, and the question can arrive before retry itself returns.
#[gpui::test]
fn explicit_sign_in_waits_for_the_requested_hosts_question(cx: &mut TestAppContext) {
    let mut app = open(cx, &[], HostStateView::SignInNeeded);
    app.terminal_live();
    keys(app.cx, "x");
    assert_eq!(label(app.cx, ANSWER).as_deref(), Some("Sign in"));
    click(app.cx, ANSWER);
    let command = app
        .helm
        .commands()
        .into_iter()
        .find(|cmd| cmd.path.ends_with("/retry"))
        .unwrap();
    assert_eq!(command.path, "/api/hosts/2/retry");
    let other = question(SECOND, 3, "Password:", false);
    publish(&mut app.helm, std::slice::from_ref(&other), &[], app.cx);
    assert!(
        !any_open(app.cx),
        "the press was for the build box, not the bench box"
    );
    publish(
        &mut app.helm,
        &[other, question(FIRST, 2, "Verification code:", false)],
        &[],
        app.cx,
    );
    assert!(answer_focused(app.cx));
}

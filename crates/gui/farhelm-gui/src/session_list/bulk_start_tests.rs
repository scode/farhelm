//! Tests of the marked bar's Restart/Replace ([`super::bulk`], its "Restart/Replace" section): when the bar offers it,
//! what each marked session gets, how a batch paces itself per host and shows each row's wait, and which rows it
//! leaves out and why. In the real main view over the helm client's fake, as the bulk Stop and Delete tests run
//! ([`super::bulk_tests`], whose fixtures these share).
//!
//! Rows are marked by real Ctrl-clicks and the bar's Restart/Replace pressed by a real click. Restart/Replace never
//! asks, so a test sees its requests straight after the click. The fake answers commands in the order the window
//! sent them, so a test answers the list's own session queries ([`settle_queries`]) before the lifecycle requests it
//! means to answer, and never answers one past a request it is holding.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::bulk::{
    CHANGED_WHILE_WAITING, DELETE_ID, ENVIRONMENT_FROZEN, NONE_CAN_START, REPLACE_WOULD_END,
    START_HELP, START_ID, START_LABEL, STOP_ID,
};
use super::environment_tests::{Provider, environment_host, registry, suspended_at};
use super::host_rail::tests::hover_help;
use super::lifecycle_tests::{drawn_inside, refuse_for_yolo};
use super::marking::COUNT_ID;
use super::marking_tests::{click_with, count, ctrl, open_list, requests, start_rename};
use super::menu_tests::{drawn, ended, label_of, snapshot};
use super::retired_tests::retired;
use super::tests::{change, listing_meta, selected, session};
use crate::launcher::launch_tests::{HostChange, start_host_change};
use crate::session_list::outcome::PROGRESS_SHOWS_AFTER;
use crate::test_support::{self, focused_id};

// ---- Helpers ---------------------------------------------------------------------------------------------------------

/// Ctrl-click each of `rows` (row selectors), marking them.
fn mark(cx: &mut VisualTestContext, rows: &[&'static str]) {
    for row in rows {
        click_with(cx, row, ctrl());
    }
}

/// Whether `command` is a Restart or a Replace, the two requests this action sends.
fn starts(command: &FakeCommand) -> bool {
    command.method == Method::Post
        && (command.path.ends_with("/restart") || command.path.ends_with("/replace"))
}

/// The commands the window sent since the last look: every one that is not a Restart or a Replace (the list's session
/// queries a selection or a listing change sends) answered with success, and the Restarts and Replaces left unanswered
/// and returned, in the order sent. The fake answers only in the order sent, so this is safe only while no earlier
/// request is held unanswered and no other command follows a returned Restart or Replace; a test where one does (a
/// Replace that opens its replacement) answers every command itself, in order.
fn settle_queries(fake: &mut FakeHelm) -> Vec<FakeCommand> {
    let mut held = Vec::new();
    for command in fake.commands() {
        if starts(&command) {
            held.push(command);
        } else {
            fake.reply(command.id, 200, None);
        }
    }
    held
}

/// `commands` as their paths, for comparing with literal routes.
fn paths(commands: &[FakeCommand]) -> Vec<&str> {
    commands
        .iter()
        .map(|command| command.path.as_str())
        .collect()
}

/// `command`'s body field `field`, or `null`.
fn field(command: &FakeCommand, field: &str) -> Value {
    command
        .body
        .as_ref()
        .and_then(|body| body.get(field))
        .cloned()
        .unwrap_or(Value::Null)
}

/// `session` as an ended session that Restart cannot resume, so its Restart/Replace is a Replace.
fn unresumable(key: &'static str) -> super::tests::Fixture {
    ended(key).restart_offer("not_captured")
}

/// Where the element drawn under `id` starts, as (top, left) in pixels: its place in reading order, since the bar wraps
/// its controls onto a second line in a narrow sidebar.
fn place(cx: &mut VisualTestContext, id: &'static str) -> (f32, f32) {
    let origin = test_support::bounds(cx, id).origin;
    (f32::from(origin.y), f32::from(origin.x))
}

// ---- When it is offered ----------------------------------------------------------------------------------------------

/// Why this matters: the action exists to start a fleet again after its host rebooted, and SPEC.md, "Session list",
/// offers it "only while every marked row displayed, retired containerized sessions aside, has ended ... or has lost
/// its terminal", so a press never mixes restarting what ended with replacing what still runs. Specified: with a
/// exited, b interrupted, c running with no terminal and e a retired session marked, the bar draws "Restart/Replace"
/// between Stop and Delete with "Restart when possible, Replace when not" as its help; marking d, which is running,
/// takes it away; unmarking d brings it back; and a bar whose marks are all retired draws none.
#[gpui::test]
fn restart_replace_is_offered_only_while_every_marked_session_has_ended(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(
        cx,
        snapshot(
            &[
                (ended("a"), 0),
                (session("b").status(json!({"state": "interrupted"})), 0),
                (session("c").terminal_missing(), 0),
                (session("d"), 0),
                (retired("e"), 0),
            ],
            false,
        ),
    );
    requests(&mut fake);
    mark(
        cx,
        &[
            "session-row-a",
            "session-row-b",
            "session-row-c",
            "session-row-e",
        ],
    );
    assert_eq!(count(cx).as_deref(), Some("4 marked"), "the premise");
    assert_eq!(label_of(cx, START_ID).as_deref(), Some(START_LABEL));
    let (stop, start, delete) = (
        place(cx, STOP_ID),
        place(cx, START_ID),
        place(cx, DELETE_ID),
    );
    assert!(
        stop < start && start < delete,
        "it comes after Stop and before Delete: {stop:?} {start:?} {delete:?}"
    );
    assert_eq!(hover_help(cx, START_ID).as_deref(), Some(START_HELP));

    mark(cx, &["session-row-d"]);
    assert!(!drawn(cx, START_ID), "a running session takes it away");
    mark(cx, &["session-row-d"]);
    assert!(drawn(cx, START_ID), "and unmarking it brings it back");

    mark(cx, &["session-row-a", "session-row-b", "session-row-c"]);
    assert_eq!(
        count(cx).as_deref(),
        Some("1 marked"),
        "the premise: only e"
    );
    assert!(!drawn(cx, START_ID), "retired marks alone offer nothing");
}

// ---- What each session gets ------------------------------------------------------------------------------------------

/// Why this matters: the hover help promises "Restart when possible, Replace when not", and a Replace discards a
/// conversation Restart would have kept (SPEC.md, "Lifecycle operations": "a Restart that silently loses the
/// conversation is a bug"). Nothing asks, so a Replace must not end what the user was not told about: the helm makes
/// the replacement before it removes the source, so a guard refusing that removal would leave two sessions. Specified:
/// with a exited and resumable, b exited and not resumable, and c exited, not resumable and listing a tab, all on one
/// host, a press asks nothing, moves focus to the marked count, and sends a's plain Restart; c's row says
/// [`REPLACE_WOULD_END`]'s line and nothing goes to c; once a's Restart is answered, b's Replace goes with the
/// nothing-alive guard and no YOLO consent; once that is refused, b's row says so, and the batch has cleared a's and b's
/// marks while c stays marked.
#[gpui::test]
fn each_session_restarts_when_it_can_and_is_replaced_only_when_nothing_runs(
    cx: &mut TestAppContext,
) {
    let (mut fake, list, cx) = open_list(
        cx,
        snapshot(
            &[
                (ended("a"), 0),
                (unresumable("b"), 0),
                (unresumable("c"), 1),
            ],
            false,
        ),
    );
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b", "session-row-c"]);
    test_support::click(cx, START_ID);
    test_support::settle(cx);
    assert!(!drawn(cx, "prompt"), "Restart/Replace asks nothing");
    assert_eq!(focused_id(cx).as_deref(), Some(COUNT_ID));
    let first = settle_queries(&mut fake);
    assert_eq!(paths(&first), ["/api/sessions/a/restart"]);
    assert_eq!(field(&first[0], "stop_if_running"), json!(false));
    assert_eq!(
        label_of(cx, "session-outcome-c").as_deref(),
        Some(format!("Replace: {REPLACE_WOULD_END}").as_str())
    );

    fake.reply(first[0].id, 200, Some(json!({})));
    test_support::settle(cx);
    let second = settle_queries(&mut fake);
    assert_eq!(paths(&second), ["/api/sessions/b/replace"]);
    assert_eq!(field(&second[0], "only_if_nothing_alive"), json!(true));
    assert_eq!(field(&second[0], "only_if_agent_ended"), json!(false));
    assert_eq!(field(&second[0], "confirm_yolo"), json!(false));

    fake.reply(
        second[0].id,
        409,
        Some(json!("the host refused this operation")),
    );
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-outcome-b").as_deref(),
        Some("Replace: the host refused this operation")
    );
    assert!(
        settle_queries(&mut fake).is_empty(),
        "nothing more was sent"
    );
    assert!(
        !super::marking_tests::stored(&list, "a", cx),
        "a's mark is cleared"
    );
    assert!(
        !super::marking_tests::stored(&list, "b", cx),
        "b's mark is cleared"
    );
    assert!(
        super::marking_tests::stored(&list, "c", cx),
        "c was never the batch's, so it stays marked"
    );
}

/// Why this matters: SPEC.md: Skip session confirmations makes each action run "with consent from the state shown by
/// that render", and a row's own Replace then closes its tabs unasked, as its Restart stops a working agent unasked;
/// the bulk action must not be more cautious than the row it stands in for once the user chose that. Specified: with
/// Skip on, a exited, not resumable and listing a tab, and b running, resumable and with no terminal, a press sends a's
/// Replace with the agent-ended guard (its tab authorized) and no nothing-alive guard; once that is answered, b's
/// Restart consents to stop its working agent.
#[gpui::test]
fn with_confirmations_skipped_each_request_carries_the_drawn_rows_consent(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(
        cx,
        snapshot(
            &[(unresumable("a"), 1), (session("b").terminal_missing(), 0)],
            true,
        ),
    );
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, START_ID);
    test_support::settle(cx);
    let sent = settle_queries(&mut fake);
    assert_eq!(paths(&sent), ["/api/sessions/a/replace"]);
    assert_eq!(field(&sent[0], "only_if_agent_ended"), json!(true));
    assert_eq!(field(&sent[0], "only_if_nothing_alive"), json!(false));
    fake.reply(sent[0].id, 409, Some(json!("refused")));
    test_support::settle(cx);
    let next = settle_queries(&mut fake);
    assert_eq!(paths(&next), ["/api/sessions/b/restart"]);
    assert_eq!(field(&next[0], "stop_if_running"), json!(true));
}

// ---- Pacing and progress ---------------------------------------------------------------------------------------------

/// Why this matters: SPEC.md: "Each bulk Stop, Delete or Restart/Replace batch runs one request at a time on each host,
/// with different hosts progressing independently"; a fleet restarted at once would trip the supervisor's management
/// limit. And a row whose Restart waits says so, so the user can see which sessions are still starting. Specified: with
/// a and b exited on one host and c exited on another, a press sends a's and c's Restarts only; two seconds later a's
/// row says "Restarting on buildbox…"; once a's is answered, b's Restart goes, and nothing else.
#[gpui::test]
fn a_batch_restarts_one_session_per_host_at_a_time_and_each_row_shows_its_wait(
    cx: &mut TestAppContext,
) {
    let (mut fake, _list, cx) = open_list(
        cx,
        snapshot(
            &[(ended("a"), 0), (ended("b"), 0), (ended("c").host(3), 0)],
            false,
        ),
    );
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b", "session-row-c"]);
    test_support::click(cx, START_ID);
    test_support::settle(cx);
    let first = settle_queries(&mut fake);
    assert_eq!(
        paths(&first),
        ["/api/sessions/a/restart", "/api/sessions/c/restart"]
    );

    test_support::advance(cx, PROGRESS_SHOWS_AFTER);
    assert_eq!(
        label_of(cx, "session-restart-progress-label-a").as_deref(),
        Some("Restarting on buildbox…")
    );

    fake.reply(first[0].id, 200, Some(json!({})));
    test_support::settle(cx);
    let next: Vec<FakeCommand> = fake.commands().into_iter().filter(starts).collect();
    assert_eq!(paths(&next), ["/api/sessions/b/restart"]);
}

// ---- What it leaves out ----------------------------------------------------------------------------------------------

/// Why this matters: SPEC.md: "Each refused row gets the ordinary action-specific not-started message", and a busy row
/// must not hold up the rest. Specified: with b's Rename in flight and a and b exited and marked, a press sends a's
/// Restart alone, and b's row says "Restart: not started, another operation was in progress. Try again."
#[gpui::test]
fn a_busy_row_says_it_was_not_started_and_the_rest_go_on(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(cx, snapshot(&[(ended("a"), 0), (ended("b"), 0)], false));
    requests(&mut fake);
    start_rename("b", cx);
    let rename = fake.commands();
    assert_eq!(rename.len(), 1, "the premise: b's Rename is in flight");
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, START_ID);
    test_support::settle(cx);
    let sent: Vec<FakeCommand> = fake.commands().into_iter().filter(starts).collect();
    assert_eq!(paths(&sent), ["/api/sessions/a/restart"]);
    assert_eq!(
        label_of(cx, "session-outcome-b").as_deref(),
        Some("Restart: not started, another operation was in progress. Try again.")
    );
}

/// Why this matters: a suspended, stopped or changing environment withholds the supervisor's lifecycle verbs, so the
/// open session's header greys Restart and Replace there; a batch must say why it skipped such a session rather than
/// send a request bound to fail. Specified: with a exited in its own suspended sandbox and b exited on another host,
/// both marked, a press sends b's Restart alone and a's row says [`ENVIRONMENT_FROZEN`]'s line.
#[gpui::test]
fn a_session_in_a_frozen_environment_is_left_out_with_its_reason(cx: &mut TestAppContext) {
    let host = environment_host(Provider::Sandbox, suspended_at(0));
    let mut state = snapshot(&[(ended("a"), 0), (ended("b").host(3), 0)], false);
    state.push(registry(&host));
    let (mut fake, _list, cx) = open_list(cx, state);
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, START_ID);
    test_support::settle(cx);
    let sent = settle_queries(&mut fake);
    assert_eq!(paths(&sent), ["/api/sessions/b/restart"]);
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some(format!("Restart: {ENVIRONMENT_FROZEN}").as_str())
    );
}

/// Why this matters: Restart and Replace start a session, and SPEC.md, "Creation", says launches and changes to the host
/// registry exclude each other; the open session's header and card wait for a host change with its reason, and the
/// bulk action must too. Specified: with Add host's probe in flight and a exited and marked, Restart/Replace's help is
/// "Wait for the host change to finish", and a click on the busy button sends no Restart, leaves a's row no line and
/// keeps its mark. (A host change that begins while rows wait is
/// `a_host_change_that_begins_mid_batch_holds_back_the_rows_still_waiting`'s.)
#[gpui::test]
fn restart_replace_waits_for_a_host_change(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(cx, snapshot(&[(ended("a"), 0)], false));
    requests(&mut fake);
    mark(cx, &["session-row-a"]);
    start_host_change(cx, HostChange::Probe);
    test_support::settle(cx);
    assert_eq!(
        hover_help(cx, START_ID).as_deref(),
        Some("Wait for the host change to finish")
    );
    test_support::click(cx, START_ID);
    test_support::settle(cx);
    assert!(
        !fake.commands().iter().any(starts),
        "nothing starts while the host change is in flight"
    );
    assert_eq!(
        label_of(cx, "session-outcome-a"),
        None,
        "the busy button did nothing"
    );
    assert!(
        super::marking_tests::stored(&list, "a", cx),
        "a stays marked"
    );
}

/// Why this matters: a batch's rows wait for their host's turn, and a host change can begin meanwhile; a row reached
/// then must not start a session while the registry changes, and must say why. This is the turn-time check, which the
/// busy button alone cannot show. Specified: with a and b exited on one host and marked, a press sends a's Restart;
/// then Add host's probe goes out; once a's Restart is answered, b sends nothing and its row says "Restart: Wait for
/// the host change to finish".
#[gpui::test]
fn a_host_change_that_begins_mid_batch_holds_back_the_rows_still_waiting(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(cx, snapshot(&[(ended("a"), 0), (ended("b"), 0)], false));
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, START_ID);
    test_support::settle(cx);
    let first = settle_queries(&mut fake);
    assert_eq!(paths(&first), ["/api/sessions/a/restart"]);
    start_host_change(cx, HostChange::Probe);
    test_support::settle(cx);
    let probe = fake.commands();
    assert_eq!(probe.len(), 1, "the premise: the probe went out");

    fake.reply(first[0].id, 200, Some(json!({})));
    test_support::settle(cx);
    assert!(
        !fake.commands().iter().any(starts),
        "b waits for the host change rather than starting"
    );
    assert_eq!(
        label_of(cx, "session-outcome-b").as_deref(),
        Some("Restart: Wait for the host change to finish")
    );
}

/// Why this matters: a row waits for its host's turn after the press, and what it would get is frozen from the bar the
/// user saw; if the session changes meanwhile (a tab opened, so its Replace would now close something), sending the
/// frozen request would act on a state the user never saw, and a Replace's guard refusal would leave two sessions.
/// Specified: with a exited and resumable and b exited and not resumable, both on one host and marked, a press sends
/// a's Restart; b then lists a terminal tab; once a's Restart is answered, b sends nothing and its row says
/// [`CHANGED_WHILE_WAITING`]'s line.
#[gpui::test]
fn a_row_that_changes_while_it_waits_is_not_sent(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(
        cx,
        snapshot(&[(ended("a"), 0), (unresumable("b"), 0)], false),
    );
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, START_ID);
    test_support::settle(cx);
    let first = settle_queries(&mut fake);
    assert_eq!(paths(&first), ["/api/sessions/a/restart"]);
    fake.push(change(
        listing_meta(&["a", "b"], 2, false),
        &[unresumable("b").tabs(1)],
        &[],
    ));
    test_support::settle(cx);

    fake.reply(first[0].id, 200, Some(json!({})));
    test_support::settle(cx);
    assert!(!fake.commands().iter().any(starts), "b is not sent");
    assert_eq!(
        label_of(cx, "session-outcome-b").as_deref(),
        Some(format!("Replace: {CHANGED_WHILE_WAITING}").as_str())
    );
}

/// Why this matters: the plan offers the action only while "at least one marked session can be restarted or replaced";
/// a button that looks ready but starts nothing would leave the user guessing. Specified: with a and b exited, not
/// resumable and each listing a tab, both marked, the bar draws Restart/Replace with [`NONE_CAN_START`] as its help, and
/// a click sends nothing and leaves no line.
#[gpui::test]
fn the_button_is_unavailable_while_no_marked_session_could_start(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(
        cx,
        snapshot(&[(unresumable("a"), 1), (unresumable("b"), 1)], false),
    );
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    assert_eq!(hover_help(cx, START_ID).as_deref(), Some(NONE_CAN_START));
    test_support::click(cx, START_ID);
    test_support::settle(cx);
    assert!(!fake.commands().iter().any(starts), "nothing is sent");
    assert_eq!(
        label_of(cx, "session-outcome-a"),
        None,
        "and nothing is said on the rows"
    );
}

/// Why this matters: the action never asks, so a double click is likely, and a second batch over rows the first holds
/// would leave each a false "another operation was in progress" line and clear the marks while the first still runs.
/// Specified: with a and b exited on one host and marked, two presses send a's Restart once; b's row has no line while
/// it waits; once a's is answered b's Restart goes, and nothing else.
#[gpui::test]
fn a_second_press_leaves_the_rows_already_starting_alone(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(cx, snapshot(&[(ended("a"), 0), (ended("b"), 0)], false));
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, START_ID);
    test_support::settle(cx);
    test_support::click(cx, START_ID);
    test_support::settle(cx);
    let first = settle_queries(&mut fake);
    assert_eq!(paths(&first), ["/api/sessions/a/restart"]);
    assert_eq!(
        label_of(cx, "session-outcome-b"),
        None,
        "b waits with no line"
    );
    assert!(
        super::marking_tests::stored(&list, "b", cx),
        "b stays marked"
    );
    fake.reply(first[0].id, 200, Some(json!({})));
    test_support::settle(cx);
    let next: Vec<FakeCommand> = fake.commands().into_iter().filter(starts).collect();
    assert_eq!(paths(&next), ["/api/sessions/b/restart"]);
}

// ---- Replace's own outcomes in a batch -------------------------------------------------------------------------------

/// Why this matters: a successful Replace removes its source, and the batch must still go on and finish; SPEC.md, a
/// completed Replace "opens its replacement only if the source is still selected". Specified: with a open, exited and
/// not resumable, and b exited and not resumable, both on one host and marked, a press sends a's Replace; once the helm
/// lists n in a's place and answers with n's session, n is the open session and b's Replace goes; once that is refused,
/// no mark is left.
#[gpui::test]
fn a_replace_that_succeeds_opens_its_replacement_and_the_batch_goes_on(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(
        cx,
        snapshot(&[(unresumable("a"), 0), (unresumable("b"), 0)], false),
    );
    requests(&mut fake);
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, START_ID);
    test_support::settle(cx);
    let first = settle_queries(&mut fake);
    assert_eq!(paths(&first), ["/api/sessions/a/replace"]);

    fake.push(change(
        listing_meta(&["n", "b"], 2, false),
        &[session("n")],
        &["a"],
    ));
    test_support::settle(cx);
    fake.reply(first[0].id, 200, Some(session("n").entry().value));
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("n"), "the replacement opens");
    // The list's own writes for the new selection go out around b's Replace, and the fake answers in order, so every
    // command is answered here in the order sent, b's Replace with a refusal.
    let after = fake.commands();
    let sent: Vec<&FakeCommand> = after.iter().filter(|command| starts(command)).collect();
    assert_eq!(
        sent.iter()
            .map(|command| command.path.as_str())
            .collect::<Vec<_>>(),
        ["/api/sessions/b/replace"]
    );
    for command in &after {
        if starts(command) {
            fake.reply(command.id, 409, Some(json!("refused")));
        } else {
            fake.reply(command.id, 200, None);
        }
    }
    test_support::settle(cx);
    settle_queries(&mut fake);
    assert_eq!(count(cx), None, "the finished batch cleared its marks");
}

/// Why this matters: a host that asks before YOLO launches refuses a YOLO launch's Replace with its marker, and the
/// row's own Replace then asks the YOLO question on its row (SPEC.md, "Lifecycle operations"); in a batch the question
/// must ask on that row too, and the host's queue must still move on. Specified: with a and b exited and not resumable
/// on one host and marked, a press sends a's Replace; once the helm refuses it for YOLO, the YOLO question's Confirm is
/// drawn in a's row and b's Replace goes.
#[gpui::test]
fn a_yolo_refusal_asks_on_its_row_and_the_queue_moves_on(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(
        cx,
        snapshot(&[(unresumable("a"), 0), (unresumable("b"), 0)], false),
    );
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, START_ID);
    test_support::settle(cx);
    let first = settle_queries(&mut fake);
    assert_eq!(paths(&first), ["/api/sessions/a/replace"]);
    refuse_for_yolo(&mut fake, cx, first[0].id);
    assert!(
        drawn_inside(cx, "prompt-confirm", "session-item-a"),
        "the YOLO question asks on a's row"
    );
    let next: Vec<FakeCommand> = fake.commands().into_iter().filter(starts).collect();
    assert_eq!(paths(&next), ["/api/sessions/b/replace"]);
}

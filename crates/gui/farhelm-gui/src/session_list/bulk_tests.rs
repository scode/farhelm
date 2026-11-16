//! Tests of bulk Stop and Delete ([`super::bulk`]), in the real main view over the helm client's fake, as the marking
//! tests run ([`super::marking_tests`], whose fixtures these share). Ported from the old
//! `e2e/tests/bulk-session-actions.spec.ts` cases about batches and the bulk question, and from the old
//! `list/lifecycle.rs` tests' rules, at reference commit `24bc0580`.
//!
//! Rows are marked by real Ctrl-clicks and the bar's Stop and Delete by real clicks; the bulk question is the prompt's
//! centered dialog (`prompt`, with `prompt-confirm` and `prompt-cancel`), and each of its rows is drawn under
//! `bulk-target-<key>`. The fake answers commands only in the order they were sent, which is also how these tests see
//! one request at a time per host: a host's next request appears only once its last one is answered.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::SessionRow;
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::bulk::{DELETE_ID, STOP_ID};
use super::delete::MANAGED_CHECKOUT;
use super::environment_tests::{Provider, deleting, environment_host, registry};
use super::host_rail::tests::filtered;
use super::marking::{CLEAR_ID, COUNT_ID, MENU_ID, MENU_TOGGLE_ID};
use super::marking_tests::{
    click_with, count, ctrl, fleet, open_list, requests, rows, start_rename, stored,
};
use super::menu_tests::{drawn, ended, label_of, push, snapshot};
use super::pins_tests::away;
use super::retired_tests::retired;
use super::row_tests::texts_within;
use super::status::{self, Live};
use super::tests::{change, listing_meta, painted, selected, session};
use crate::components::Role;
use crate::focus;
use crate::session;
use crate::test_support::{self, focused_id, frame, show_next_frame};

// ---- Helpers ---------------------------------------------------------------------------------------------------------

/// The commands the window sent since the last look, left unanswered, as (method, path).
fn paths(commands: &[FakeCommand]) -> Vec<(Method, &str)> {
    commands
        .iter()
        .map(|command| (command.method, command.path.as_str()))
        .collect()
}

/// `POST /api/sessions/<key>/stop`, as [`paths`] shows it.
fn stop(key: &str) -> (Method, String) {
    (Method::Post, format!("/api/sessions/{key}/stop"))
}

/// A Stop's body with these preconditions.
fn stop_body(refuse_if_tabs_open: bool, refuse_if_agent_running: bool) -> Value {
    json!({
        "refuse_if_tabs_open": refuse_if_tabs_open,
        "refuse_if_agent_running": refuse_if_agent_running,
    })
}

/// [`paths`] with owned paths, for comparing against [`stop`] and literal Delete paths.
fn owned(commands: &[FakeCommand]) -> Vec<(Method, String)> {
    paths(commands)
        .into_iter()
        .map(|(method, path)| (method, path.to_owned()))
        .collect()
}

/// `DELETE <path>`, as [`owned`] shows it.
fn delete(path: &str) -> (Method, String) {
    (Method::Delete, path.to_owned())
}

/// `commands` split into the Stops and Deletes and everything else (the list's session query, which a selection
/// change sends again), each in the order sent, so a test answers both in the order the fake requires.
fn lifecycle(commands: Vec<FakeCommand>) -> (Vec<FakeCommand>, Vec<FakeCommand>) {
    commands
        .into_iter()
        .partition(|command| command.method == Method::Delete || command.path.ends_with("/stop"))
}

/// Answer each of `others` as done, as the helm answers the list's session query.
fn answer(fake: &mut FakeHelm, others: &[FakeCommand]) {
    for command in others {
        fake.reply(command.id, 200, None);
    }
}

/// Answer `command` as the helm answers a Stop or a Delete that did what it was asked.
fn done(fake: &mut FakeHelm, command: &FakeCommand) {
    let body = (command.method == Method::Post).then(|| json!({}));
    fake.reply(command.id, 200, body);
}

/// The text a frame shows, every element's own.
fn texts(cx: &mut VisualTestContext) -> Vec<String> {
    painted(&frame(cx))
}

/// The keys of a to e whose row the bulk question draws, in that order.
fn question_rows(cx: &mut VisualTestContext) -> Vec<&'static str> {
    [
        ("a", "bulk-target-a"),
        ("b", "bulk-target-b"),
        ("c", "bulk-target-c"),
        ("d", "bulk-target-d"),
        ("e", "bulk-target-e"),
    ]
    .into_iter()
    .filter(|(_, row)| drawn(cx, row))
    .map(|(key, _)| key)
    .collect()
}

/// The texts painted in the status cell of the bulk question's row for `key` (`bulk-target-word-<key>`), in paint
/// order: a live status's word, or an ended status's words.
fn status_words(cx: &mut VisualTestContext, key: &str) -> Vec<String> {
    texts_within(&frame(cx), &format!("bulk-target-word-{key}"))
        .into_iter()
        .map(|text| text.text.to_string())
        .collect()
}

/// Ctrl-click each of `rows` (row selectors), marking them.
fn mark(cx: &mut VisualTestContext, rows: &[&'static str]) {
    for row in rows {
        click_with(cx, row, ctrl());
    }
}

// ---- Batches ---------------------------------------------------------------------------------------------------------

/// Why this matters: SPEC.md: "Each bulk Stop or Delete batch runs one request at a time on each host, with different
/// hosts progressing independently; a second batch started while the first is still running has its own pace", and
/// pending rows "do not prevent opening sessions, marking other rows, Clear"; the old spec's "Stop paces nine rows on
/// one host while another progresses". A burst would trip the supervisor's management limit, and a batch that held up
/// everything after it would freeze the list. Specified: with a, b and c on one host and d and e on another, all marked
/// and listing no tabs, Stop asks nothing and sends a's and d's Stops only, each refusing a tab opened since; the bar's
/// Stop is the old toolbar's 26 pixels high; Clear empties the bar, and f marked then stopped sends f's Stop at once
/// beside a's; each answer then sends that host's next row in the order drawn (b after a, e after d, c after b), and
/// f's batch, finished first, clears f's mark.
#[gpui::test]
fn a_batch_sends_one_request_per_host_and_a_second_batch_keeps_its_own_pace(
    cx: &mut TestAppContext,
) {
    let (mut fake, list, cx) = open_list(
        cx,
        rows(vec![
            session("a"),
            session("b"),
            session("c"),
            session("d").host(3),
            session("e").host(3),
            session("f"),
        ]),
    );
    requests(&mut fake);
    mark(
        cx,
        &[
            "session-row-a",
            "session-row-b",
            "session-row-c",
            "session-row-d",
            "session-row-e",
        ],
    );
    assert_eq!(count(cx).as_deref(), Some("5 marked"), "the premise");
    assert_eq!(
        f32::from(test_support::bounds(cx, STOP_ID).size.height),
        26.
    );

    test_support::click(cx, STOP_ID);
    test_support::settle(cx);
    assert!(
        !drawn(cx, "prompt"),
        "no target lists a tab, so nothing asks"
    );
    let first = fake.commands();
    assert_eq!(owned(&first), [stop("a"), stop("d")]);
    for command in &first {
        assert_eq!(command.body, Some(stop_body(true, false)));
    }

    test_support::click(cx, CLEAR_ID);
    test_support::settle(cx);
    assert_eq!(count(cx), None, "Clear works while the batch runs");
    mark(cx, &["session-row-f"]);
    test_support::click(cx, STOP_ID);
    test_support::settle(cx);
    let second = fake.commands();
    assert_eq!(
        owned(&second),
        [stop("f")],
        "the second batch does not wait for a's host"
    );

    done(&mut fake, &first[0]);
    test_support::settle(cx);
    let after_a = fake.commands();
    assert_eq!(owned(&after_a), [stop("b")]);
    done(&mut fake, &first[1]);
    test_support::settle(cx);
    let after_d = fake.commands();
    assert_eq!(owned(&after_d), [stop("e")]);
    done(&mut fake, &second[0]);
    test_support::settle(cx);
    assert!(!stored(&list, "f", cx), "f's batch cleared its own target");
    assert_eq!(fake.commands().len(), 0);
    done(&mut fake, &after_a[0]);
    test_support::settle(cx);
    let after_b = fake.commands();
    assert_eq!(owned(&after_b), [stop("c")]);
    done(&mut fake, &after_d[0]);
    done(&mut fake, &after_b[0]);
    test_support::settle(cx);
    assert_eq!(fake.commands().len(), 0, "every row was sent once");
}

/// Why this matters: SPEC.md: "A failure remains on its own row without undoing successful siblings. Each refused row
/// gets the ordinary action-specific not-started message, including a row engaged by its own question or rename", and a
/// batch "clears its own targets when every completion has finished, including when some rows failed or were refused";
/// the old spec's "Stop reports an engaged row and runs its sibling". Specified: with b's Rename in flight and a, b and
/// c marked, Stop sends a's Stop alone and b's row says "Stop: not started, another operation was in progress. Try
/// again."; the helm refusing a's Stop leaves "Stop: the agent is busy" on a's row and sends c's; once c's is answered
/// no mark is left, and both lines stay.
#[gpui::test]
fn a_refused_row_keeps_its_line_and_its_siblings_go_on(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(cx, fleet(&["a", "b", "c"]));
    requests(&mut fake);
    start_rename("b", cx);
    let rename = fake.commands();
    assert_eq!(rename.len(), 1, "the premise: b's Rename is in flight");
    mark(cx, &["session-row-a", "session-row-b", "session-row-c"]);

    test_support::click(cx, STOP_ID);
    test_support::settle(cx);
    let first = fake.commands();
    assert_eq!(owned(&first), [stop("a")]);
    let not_started = "Stop: not started, another operation was in progress. Try again.";
    assert_eq!(
        label_of(cx, "session-outcome-b").as_deref(),
        Some(not_started)
    );

    fake.reply(rename[0].id, 200, None);
    fake.reply(first[0].id, 409, Some(json!("the agent is busy")));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some("Stop: the agent is busy")
    );
    let next = fake.commands();
    assert_eq!(
        owned(&next),
        [stop("c")],
        "the refusal stopped nothing else"
    );

    done(&mut fake, &next[0]);
    test_support::settle(cx);
    for key in ["a", "b", "c"] {
        assert!(!stored(&list, key, cx), "{key}'s mark was the batch's");
    }
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some("Stop: the agent is busy")
    );
    assert_eq!(
        label_of(cx, "session-outcome-b").as_deref(),
        Some(not_started)
    );
}

/// Why this matters: SPEC.md: a batch "that has been refused" for every row still settles, and "a second batch ... has
/// its own pace", but a row another batch still holds is that batch's: sending it twice would delete or stop it twice;
/// the old spec's "direct Delete" case pressed Delete again while the first batch held both rows. Specified: with a and
/// b ended and listing no tabs on one host, Delete asks nothing and sends a's Delete with `only_if_nothing_alive`; a
/// second Delete sends nothing and gives both rows "Delete: not started, another operation was in progress. Try
/// again.", and that refused batch clears its own unchanged marks; answering a's Delete still sends b's, from the first
/// batch, which clears b's line as it starts.
#[gpui::test]
fn a_second_batch_skips_the_rows_the_first_still_holds(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(
        cx,
        rows(vec![
            session("o"),
            ended("a").unmanaged(),
            ended("b").unmanaged(),
        ]),
    );
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);

    test_support::click(cx, DELETE_ID);
    test_support::settle(cx);
    assert!(!drawn(cx, "prompt"), "nothing alive, so nothing asks");
    let first = fake.commands();
    assert_eq!(
        owned(&first),
        [delete("/api/sessions/a?only_if_nothing_alive=true")]
    );

    test_support::click(cx, DELETE_ID);
    test_support::settle(cx);
    assert_eq!(fake.commands().len(), 0, "no row is sent twice");
    let not_started = "Delete: not started, another operation was in progress. Try again.";
    for outcome in ["session-outcome-a", "session-outcome-b"] {
        assert_eq!(
            label_of(cx, outcome).as_deref(),
            Some(not_started),
            "{outcome}"
        );
    }
    assert!(!stored(&list, "a", cx) && !stored(&list, "b", cx));

    done(&mut fake, &first[0]);
    test_support::settle(cx);
    assert_eq!(
        owned(&fake.commands()),
        [delete("/api/sessions/b?only_if_nothing_alive=true")]
    );
    assert_eq!(label_of(cx, "session-outcome-b"), None);
}

/// Why this matters: the queues and the holds belong to the UI session (SPEC_impl.md, "One failure path: the reset"):
/// a queued Delete that outlived a reset would hold the automatic selection back for good, and a batch that went on
/// sending would act for a connection that is gone. Specified: with a open, a's Delete in flight and b's queued on the
/// same host (both ended), a broken channel and a new snapshot listing b and c open b, the newest visit, show no
/// pending Delete on b, and send no Delete of b afterwards.
#[gpui::test]
fn a_reset_drops_the_queued_rows(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(
        cx,
        rows(vec![
            ended("a").unmanaged(),
            ended("b").unmanaged(),
            session("c"),
        ]),
    );
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, DELETE_ID);
    test_support::settle(cx);
    assert_eq!(
        owned(&fake.commands()),
        [delete("/api/sessions/a?only_if_nothing_alive=true")],
        "the premise: b is queued"
    );
    assert!(
        label_of(cx, "session-delete-progress-b").is_some(),
        "the premise: b is held"
    );

    fake.break_channel();
    cx.run_until_parked();
    fake.connect(rows(vec![ended("b").unmanaged(), session("c")]));
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("b"));
    assert_eq!(label_of(cx, "session-delete-progress-b"), None);
    let mut after = requests(&mut fake);
    test_support::settle(cx);
    after.extend(requests(&mut fake));
    assert!(
        after.iter().all(|(method, _, _)| *method != Method::Delete),
        "b's Delete went with the reset: {after:?}"
    );
}

/// Why this matters: SPEC.md: "Accepted queued rows remain pending until their completions finish", and an open
/// lifecycle confirmation or a pending request refuses another lifecycle action; the old app reserved every admitted
/// row at acceptance. A queued row that looked idle would take a second Stop or Delete that the batch then runs into.
/// Specified: with a and b on one host and listing no tabs, Stop sends a's and holds b, which shows "Stopping…" and
/// whose own menu's Stop is unavailable (the menu stays open on it); a's answer sends b's Stop in the hold's place,
/// still "Stopping…"; b's answer leaves b idle, and its menu's Stop then sends at once.
#[gpui::test]
fn a_queued_row_is_pending_until_its_turn_and_free_after_it(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(cx, fleet(&["a", "b"]));
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, STOP_ID);
    test_support::settle(cx);
    let first = fake.commands();
    assert_eq!(owned(&first), [stop("a")], "the premise: b is queued");
    assert_eq!(
        label_of(cx, "session-stop-progress-b").as_deref(),
        Some("Stopping…")
    );
    test_support::click(cx, "session-menu-b");
    test_support::click(cx, "session-menu-stop");
    assert!(
        drawn(cx, "session-menu"),
        "b's Stop is unavailable while queued"
    );
    test_support::keys(cx, "escape");
    assert_eq!(fake.commands().len(), 0);

    done(&mut fake, &first[0]);
    test_support::settle(cx);
    let turn = fake.commands();
    assert_eq!(owned(&turn), [stop("b")]);
    assert_eq!(
        label_of(cx, "session-stop-progress-b").as_deref(),
        Some("Stopping…")
    );

    done(&mut fake, &turn[0]);
    test_support::settle(cx);
    assert_eq!(label_of(cx, "session-stop-progress-b"), None);
    test_support::click(cx, "session-menu-b");
    test_support::click(cx, "session-menu-stop");
    test_support::settle(cx);
    assert_eq!(owned(&fake.commands()), [stop("b")], "b is free again");
}

/// Why this matters: the old spec's "Stop reports an engaged row and runs its sibling" pressed the bar's Stop while a
/// row's own Delete question was open, through a scripted click that left the question open. In the app a press on the
/// bar closes that anchored question first (a press outside it is its Cancel), so the row is no longer engaged and the
/// bulk Stop must take it like any other. Specified: with a and b marked and b's own Delete question open, a click on
/// the bar's Stop closes the question, sends a's Stop, and then b's once a's is answered; b has no not-started line.
#[gpui::test]
fn a_press_on_the_bar_closes_a_rows_own_question_and_takes_the_row(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(cx, fleet(&["a", "b"]));
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, "session-menu-b");
    test_support::click(cx, "session-menu-delete");
    test_support::settle(cx);
    assert!(
        drawn(cx, "prompt"),
        "the premise: b's Delete question is open"
    );

    test_support::click(cx, STOP_ID);
    test_support::settle(cx);
    assert!(!drawn(cx, "prompt"), "the press closed b's question");
    let first = fake.commands();
    assert_eq!(owned(&first), [stop("a")]);
    done(&mut fake, &first[0]);
    test_support::settle(cx);
    assert_eq!(owned(&fake.commands()), [stop("b")]);
    assert_eq!(label_of(cx, "session-outcome-b"), None);
}

// ---- Completion and later choices ------------------------------------------------------------------------------------

/// A marking choice the user makes while a's Stop is in flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Later {
    /// Clear, mark b and c, and open the bulk Delete question about them.
    AnotherQuestion,
    /// Unmark a and mark it again.
    Remarked,
    /// Open the marked menu over the unchanged mark.
    Menu,
    /// Open the bulk Delete question over the unchanged mark.
    SameQuestion,
}

/// The old spec's "completion preserves a later intent" case for `later`: a's Stop (no tabs, so unasked) is held, the
/// user makes the `later` choice, then the helm refuses the Stop. The refusal shows on a's row, and the marks, the menu
/// or the question the later choice left are untouched by the batch's completion. The question over a then still sends
/// a's Delete with the guard it drew (a live agent: none).
fn completion_preserves(later: Later, cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(cx, fleet(&["a", "b", "c"]));
    requests(&mut fake);
    mark(cx, &["session-row-a"]);
    test_support::click(cx, STOP_ID);
    test_support::settle(cx);
    let held = fake.commands();
    assert_eq!(owned(&held), [stop("a")], "the premise: a's Stop is held");

    match later {
        Later::AnotherQuestion => {
            test_support::click(cx, CLEAR_ID);
            mark(cx, &["session-row-b", "session-row-c"]);
            test_support::click(cx, DELETE_ID);
        }
        Later::Remarked => mark(cx, &["session-row-a", "session-row-a"]),
        Later::Menu => test_support::click(cx, MENU_TOGGLE_ID),
        Later::SameQuestion => test_support::click(cx, DELETE_ID),
    }
    test_support::settle(cx);
    let marked: &[&str] = if later == Later::AnotherQuestion {
        &["b", "c"]
    } else {
        &["a"]
    };
    let questioned = matches!(later, Later::AnotherQuestion | Later::SameQuestion);
    assert_eq!(drawn(cx, "prompt"), questioned, "the premise");
    if questioned {
        assert_eq!(question_rows(cx), marked, "the premise: what it asks about");
    }
    assert_eq!(drawn(cx, MENU_ID), later == Later::Menu, "the premise");

    fake.reply(held[0].id, 409, Some(json!("the first batch was refused")));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some("Stop: the first batch was refused"),
        "the premise: the Stop settled"
    );
    for key in ["a", "b", "c"] {
        assert_eq!(stored(&list, key, cx), marked.contains(&key), "{key}");
    }
    assert_eq!(drawn(cx, "prompt"), questioned, "the question stays open");
    assert_eq!(
        drawn(cx, MENU_ID),
        later == Later::Menu,
        "the menu stays open"
    );

    if later == Later::SameQuestion {
        test_support::click(cx, "prompt-confirm");
        test_support::settle(cx);
        assert_eq!(
            owned(&fake.commands()),
            [delete("/api/sessions/a")],
            "the preserved question still acts on a, with the live agent's guard it drew"
        );
    }
}

/// Why this matters: SPEC.md: a batch clears its own targets "only if the user has made no newer marking choice or
/// opened a marked-actions menu or bulk question", and "set equality cannot identify that ownership" (the old view's
/// interaction revision). Specified: a later bulk question about other marks keeps those marks and the question.
#[gpui::test]
fn completion_keeps_the_marks_of_a_later_question(cx: &mut TestAppContext) {
    completion_preserves(Later::AnotherQuestion, cx);
}

/// Why this matters: see [`completion_keeps_the_marks_of_a_later_question`]; the same keys marked again are a new
/// choice, which set equality would miss. Specified: unmarking and marking a again keeps a marked.
#[gpui::test]
fn completion_keeps_a_target_marked_again(cx: &mut TestAppContext) {
    completion_preserves(Later::Remarked, cx);
}

/// Why this matters: see [`completion_keeps_the_marks_of_a_later_question`]; a menu opened over unchanged marks is a
/// choice in flight. Specified: the marked menu opened over a stays open and a stays marked.
#[gpui::test]
fn completion_keeps_the_marks_under_a_later_menu(cx: &mut TestAppContext) {
    completion_preserves(Later::Menu, cx);
}

/// Why this matters: see [`completion_keeps_the_marks_of_a_later_question`]; clearing a would make the open question's
/// answer act on nothing. Specified: a bulk Delete question over a stays open, a stays marked, and Confirm sends a's
/// Delete with no precondition flag.
#[gpui::test]
fn completion_keeps_the_marks_of_a_later_question_about_them(cx: &mut TestAppContext) {
    completion_preserves(Later::SameQuestion, cx);
}

// ---- Automatic selection ---------------------------------------------------------------------------------------------

/// Why this matters: SPEC.md: "Automatic selection after Delete waits until every admitted Delete has settled, queued
/// rows and other hosts included, and never opens a session that is still being deleted"; queued Deletes are not in the
/// operation store until they are sent, so the store alone would let the selection open one; the old spec's "cross-host
/// selection exclusion". Specified: with a open and a and b ended on one host, c ended on another and d alive and
/// unmarked, Delete sends a's and c's; a's Delete done and a gone from a complete listing opens nothing while b is
/// queued; c's done and gone opens nothing while b's is in flight; b's done and gone opens d.
#[gpui::test]
fn automatic_selection_waits_for_every_admitted_delete(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(
        cx,
        rows(vec![
            ended("a").unmanaged(),
            ended("b").unmanaged(),
            ended("c").unmanaged().host(3),
            session("d"),
        ]),
    );
    requests(&mut fake);
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    mark(cx, &["session-row-a", "session-row-b", "session-row-c"]);
    test_support::click(cx, DELETE_ID);
    test_support::settle(cx);
    let first = fake.commands();
    assert_eq!(
        owned(&first),
        [
            delete("/api/sessions/a?only_if_nothing_alive=true"),
            delete("/api/sessions/c?only_if_nothing_alive=true"),
        ]
    );
    away(cx);

    done(&mut fake, &first[0]);
    fake.push(change(
        listing_meta(&["b", "c", "d"], 3, false),
        &[],
        &["a"],
    ));
    test_support::settle(cx);
    assert_eq!(selected(cx), None, "b is still queued");
    let (b, queries) = lifecycle(fake.commands());
    assert_eq!(
        owned(&b),
        [delete("/api/sessions/b?only_if_nothing_alive=true")]
    );

    // In the order sent: c's Delete went before the query and b's Delete.
    done(&mut fake, &first[1]);
    answer(&mut fake, &queries);
    fake.push(change(listing_meta(&["b", "d"], 2, false), &[], &["c"]));
    test_support::settle(cx);
    assert_eq!(selected(cx), None, "b's Delete is in flight");
    let (none, queries) = lifecycle(fake.commands());
    assert!(none.is_empty(), "nothing else is sent: {:?}", paths(&none));

    done(&mut fake, &b[0]);
    answer(&mut fake, &queries);
    fake.push(change(listing_meta(&["d"], 1, false), &[], &["b"]));
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("d"));
}

/// Why this matters: SPEC.md: "Pending Stop batches do not delay automatic selection"; only Deletes hold it back, and a
/// queued Stop is held in the operation store as a Stop, which must not count as one. Specified: with a open and a and
/// b stopping on one host (a's in flight, b's queued), a complete listing without a opens another session at once.
#[gpui::test]
fn a_stop_batch_holds_no_automatic_selection_back(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(cx, fleet(&["a", "b", "c"]));
    requests(&mut fake);
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, STOP_ID);
    test_support::settle(cx);
    assert_eq!(
        owned(&fake.commands()),
        [stop("a")],
        "the premise: b is queued"
    );
    away(cx);

    fake.push(change(listing_meta(&["b", "c"], 2, false), &[], &["a"]));
    test_support::settle(cx);
    let opened = selected(cx);
    assert!(
        matches!(opened.as_deref(), Some("b" | "c")),
        "another session opened: {opened:?}"
    );
}

// ---- The question ----------------------------------------------------------------------------------------------------

/// Why this matters: SPEC.md: bulk Stop and Delete "ask once when any target would need its ordinary confirmation", a
/// row's Stop asks only about tabs, and "Stop names the agent and terminal tabs it ends, with a separate no-tabs
/// precondition for every target whose displayed state showed no tabs"; after confirming, "keyboard focus returns to
/// the marked count". Specified: Stop of a lone tabless a asks nothing; Stop of a, b (two tabs) and c (ended, no tabs)
/// asks "Stop 3 sessions?" with focus on its Confirm and each row's own consequence; Confirm sends a's Stop refusing a
/// new tab, then b's with no precondition, then c's refusing a new tab and a running agent, with focus on "N marked"
/// meanwhile and on the order's title once the batch has cleared the marks and the bar.
#[gpui::test]
fn a_bulk_stop_asks_about_tabs_and_freezes_each_rows_preconditions(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(
        cx,
        snapshot(
            &[(session("a"), 0), (session("b"), 2), (ended("c"), 0)],
            false,
        ),
    );
    requests(&mut fake);
    mark(cx, &["session-row-a"]);
    test_support::click(cx, STOP_ID);
    test_support::settle(cx);
    assert!(!drawn(cx, "prompt"), "a lists no tab");
    let alone = fake.commands();
    assert_eq!(owned(&alone), [stop("a")]);
    done(&mut fake, &alone[0]);
    test_support::settle(cx);

    mark(cx, &["session-row-a", "session-row-b", "session-row-c"]);
    test_support::click(cx, STOP_ID);
    test_support::settle(cx);
    assert!(drawn(cx, "prompt"));
    assert_eq!(
        fake.commands().len(),
        0,
        "nothing is sent before the answer"
    );
    assert_eq!(question_rows(cx), ["a", "b", "c"]);
    let shown = texts(cx);
    for words in [
        "Stop 3 sessions?",
        "Stop the agent?",
        "Stop the agent and close 2 terminal tabs?",
        "Stop this session?",
    ] {
        assert!(shown.iter().any(|text| text == words), "{words}: {shown:?}");
    }
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));

    test_support::click(cx, "prompt-confirm");
    test_support::settle(cx);
    assert!(!drawn(cx, "prompt"));
    assert_eq!(focused_id(cx).as_deref(), Some(COUNT_ID));
    let mut bodies = Vec::new();
    for key in ["a", "b", "c"] {
        let sent = fake.commands();
        assert_eq!(owned(&sent), [stop(key)], "{key} in its turn");
        bodies.push(sent[0].body.clone());
        done(&mut fake, &sent[0]);
        test_support::settle(cx);
    }
    assert_eq!(
        bodies,
        [
            Some(stop_body(true, false)),
            Some(stop_body(false, false)),
            Some(stop_body(true, true)),
        ]
    );
    assert_eq!(count(cx), None, "the batch cleared its targets");
    assert_eq!(focused_id(cx).as_deref(), Some("session-order"));
}

/// Why this matters: SPEC.md: the bulk question "lists one row per target with status, name, full host and what the
/// action will do, with the same conditional managed-checkout consequence for Delete", and "Cancel, Close and Escape
/// dismiss their current question without clearing marks"; the old app returned focus to the ⋯ after a Cancel. The rows
/// are what the answer authorizes, so each must say what will happen to its own session, and a keyboard user must be
/// able to scroll every row into view before answering, as the old body's "Sessions affected" region let them (SPEC.md
/// keeps Tab within a modal's "keyboard scroll regions"). Specified: Delete of a (running, in a managed checkout), b
/// (exited, on a host named `a-very-long-host.example.test`) and c (status unknown) asks "Delete 3 sessions?" with
/// focus on Confirm, rows saying "Running", "Exited (code 0)" and "Unknown", each title, b's full host, each row's
/// Delete consequence and the managed checkout's line once; Tab from Confirm reaches the text's scroll stop, a region
/// named "Sessions affected"; Cancel then sends nothing, keeps the three marks and returns focus to the ⋯, the control
/// the question names for its close, though the bar's Delete asked it and focus was last on the stop. (The stop is
/// there whether or not the rows overflow, under the dialog frame's rule, tested with it; Skip session confirmations
/// is `skip_session_confirmations_runs_bulk_stop_and_delete_unasked`'s.)
#[gpui::test]
fn a_bulk_delete_question_lists_each_target_and_cancel_keeps_the_marks(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(
        cx,
        snapshot(
            &[
                (session("a"), 0),
                (
                    ended("b")
                        .unmanaged()
                        .host_name("a-very-long-host.example.test"),
                    0,
                ),
                (
                    session("c").status(json!({"state": "unknown"})).unmanaged(),
                    0,
                ),
            ],
            false,
        ),
    );
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b", "session-row-c"]);
    test_support::click(cx, DELETE_ID);
    test_support::settle(cx);
    assert!(drawn(cx, "prompt"));
    assert_eq!(question_rows(cx), ["a", "b", "c"]);
    let shown = texts(cx);
    for words in [
        "Delete 3 sessions?",
        "Running",
        "Exited (code 0)",
        "Unknown",
        "Still running — deleting kills the agent:",
        "Delete anyway:",
        "Status unknown — the agent may still be running and will be killed:",
    ] {
        assert!(shown.iter().any(|text| text == words), "{words}: {shown:?}");
    }
    assert_eq!(
        shown
            .iter()
            .filter(|text| *text == MANAGED_CHECKOUT)
            .count(),
        1,
        "only a is in a managed checkout"
    );
    for (id, words) in [
        ("bulk-target-title-a", "a"),
        ("bulk-target-host-b", "a-very-long-host.example.test"),
    ] {
        assert_eq!(label_of(cx, id).as_deref(), Some(words), "{id}");
    }
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));

    test_support::keys(cx, "tab");
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-body-stop"));
    let stop = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("prompt-body-stop"))
        .expect("the stop is drawn");
    assert_eq!(
        (stop.role, stop.label.as_deref()),
        (Some(Role::Region), Some("Sessions affected"))
    );

    test_support::click(cx, "prompt-cancel");
    test_support::settle(cx);
    assert!(!drawn(cx, "prompt"));
    assert_eq!(fake.commands().len(), 0);
    assert_eq!(count(cx).as_deref(), Some("3 marked"));
    assert_eq!(focused_id(cx).as_deref(), Some(MENU_TOGGLE_ID));
    for key in ["a", "b", "c"] {
        assert!(stored(&list, key, cx), "{key}");
    }
}

/// Why this matters: SPEC.md: Skip session confirmations "skips session row, header and bulk Stop, session row and bulk
/// Delete", and each runs "with consent from the state shown by that render"; the old spec's "direct Delete skips the
/// live question". Specified: with Skip on, Stop of a (one tab) and b asks nothing and sends a's Stop with no
/// precondition; once both are done, Delete of a and b asks nothing and sends a's Delete with no precondition.
#[gpui::test]
fn skip_session_confirmations_runs_bulk_stop_and_delete_unasked(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) =
        open_list(cx, snapshot(&[(session("a"), 1), (session("b"), 0)], true));
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, STOP_ID);
    test_support::settle(cx);
    assert!(!drawn(cx, "prompt"));
    let a = fake.commands();
    assert_eq!(owned(&a), [stop("a")]);
    assert_eq!(a[0].body, Some(stop_body(false, false)));
    done(&mut fake, &a[0]);
    test_support::settle(cx);
    let b = fake.commands();
    assert_eq!(owned(&b), [stop("b")]);
    done(&mut fake, &b[0]);
    test_support::settle(cx);

    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, DELETE_ID);
    test_support::settle(cx);
    assert!(!drawn(cx, "prompt"));
    assert_eq!(owned(&fake.commands()), [delete("/api/sessions/a")]);
}

/// Why this matters: SPEC.md: "An accepted changed target rewords the question and replaces its opening; authoritative
/// disappearance can remove it, while a filtered, partial, failed or non-authoritative omission cannot", and
/// "answer-time admission may only remove targets no longer displayed or marked"; the old spec's "Delete rewords and
/// shrinks without gaining targets". The reword is in place, as the old question's was, so focus stays on the answer
/// the user was on rather than jumping while they read. Specified: with a, b and d asked about (all running) and c
/// unmarked, b exiting rewords b's row to "Delete anyway:" with focus still on Confirm; a complete listing without d
/// removes d's row ("Delete 2 sessions?"); a filtered listing that leaves a out keeps a's row; once the new words are
/// on screen, Confirm sends b's Delete alone, with the reworded row's `only_if_nothing_alive`, since a is no longer
/// shown.
#[gpui::test]
fn a_changed_target_rewords_the_question_which_never_gains_one(cx: &mut TestAppContext) {
    let unmanaged = |key| session(key).unmanaged();
    let (mut fake, _list, cx) = open_list(
        cx,
        rows(vec![
            unmanaged("a"),
            unmanaged("b"),
            unmanaged("c"),
            unmanaged("d"),
        ]),
    );
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b", "session-row-d"]);
    test_support::click(cx, DELETE_ID);
    test_support::settle(cx);
    assert_eq!(question_rows(cx), ["a", "b", "d"], "the premise");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-confirm"),
        "the premise"
    );
    away(cx);

    push(
        &mut fake,
        &["a", "b", "c", "d"],
        &[(ended("b").unmanaged(), 0)],
        cx,
    );
    assert!(drawn(cx, "prompt"));
    assert_eq!(question_rows(cx), ["a", "b", "d"]);
    assert!(texts(cx).iter().any(|text| text == "Delete anyway:"));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-confirm"),
        "focus stayed where it was"
    );

    fake.push(change(
        listing_meta(&["a", "b", "c"], 3, false),
        &[],
        &["d"],
    ));
    test_support::settle(cx);
    assert_eq!(question_rows(cx), ["a", "b"]);
    assert!(texts(cx).iter().any(|text| text == "Delete 2 sessions?"));

    fake.push(change(filtered(&["b", "c"], 3, 2), &[], &["a"]));
    test_support::settle(cx);
    assert!(
        !drawn(cx, "session-row-a"),
        "the premise: the filter hides a"
    );
    assert_eq!(
        question_rows(cx),
        ["a", "b"],
        "a filtered omission removes nothing"
    );
    let (sent, _) = lifecycle(fake.commands());
    assert!(
        sent.is_empty(),
        "and dispatches nothing: {:?}",
        paths(&sent)
    );

    show_next_frame(cx);
    test_support::click(cx, "prompt-confirm");
    test_support::settle(cx);
    assert_eq!(
        owned(&lifecycle(fake.commands()).0),
        [delete("/api/sessions/b?only_if_nothing_alive=true")]
    );
}

/// Why this matters: a press on the bar's Stop or Delete must count its targets from what the frame drew, and only
/// marked rows shown are drawn; a hidden mark acted on would delete a session the user cannot see. Specified: with a
/// and b marked and a filtered listing hiding b, the bar says "1 marked" and Delete of a (nothing alive) sends a's
/// Delete alone.
#[gpui::test]
fn a_batch_acts_only_on_the_marks_shown(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(
        cx,
        rows(vec![
            session("o"),
            ended("a").unmanaged(),
            ended("b").unmanaged(),
        ]),
    );
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    away(cx);
    fake.push(change(filtered(&["o", "a"], 3, 2), &[], &["b"]));
    test_support::settle(cx);
    assert_eq!(count(cx).as_deref(), Some("1 marked"), "the premise");

    test_support::click(cx, DELETE_ID);
    test_support::settle(cx);
    assert_eq!(
        owned(&fake.commands()),
        [delete("/api/sessions/a?only_if_nothing_alive=true")]
    );
}

/// Why this matters: SPEC.md: "authoritative disappearance can remove" a target, and a question about nothing would
/// authorize nothing; the old reconcile closed the question once no target was left. The close runs before the frame
/// that drops the bar, so focus goes to the ⋯ still on screen and, once it goes, recovers to the open session's terminal
/// (SPEC.md, "Where focus rests"); a fallback to what held focus at opening would land on a row instead. Specified:
/// with c open and focused on its row, a and b marked, and the macOS click rule (the bar's Delete takes no focus, so
/// row c holds it when the question opens), the Delete question asks about a and b; a complete listing without either
/// closes it, sends nothing, and leaves focus on the terminal, not on row c.
#[gpui::test]
fn the_question_closes_once_no_target_is_left(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(cx, fleet(&["c", "a", "b"]));
    requests(&mut fake);
    // Marking c and unmarking it again leaves focus on c's row, which survives the listing below.
    mark(
        cx,
        &[
            "session-row-a",
            "session-row-b",
            "session-row-c",
            "session-row-c",
        ],
    );
    assert_eq!(count(cx).as_deref(), Some("2 marked"), "the premise");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-row-c"),
        "the premise"
    );
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    test_support::click(cx, DELETE_ID);
    test_support::settle(cx);
    assert_eq!(question_rows(cx), ["a", "b"], "the premise");
    away(cx);

    fake.push(change(listing_meta(&["c"], 1, false), &[], &["a", "b"]));
    test_support::settle(cx);
    assert!(!drawn(cx, "prompt"));
    let (sent, _) = lifecycle(fake.commands());
    assert!(sent.is_empty(), "{:?}", paths(&sent));
    assert_eq!(selected(cx).as_deref(), Some("c"), "c stays open");
    assert!(
        drawn(cx, "session-row-c"),
        "the premise: the opener's row is still drawn"
    );
    assert!(
        test_support::terminal_focused(cx),
        "focus is on the terminal: {:?}",
        focused_id(cx)
    );
}

/// Why this matters: the question's rows say each target's status, and the old question replaced itself in place on
/// any change it drew, its status words included, so a row never named a status its session had left. An agent at work
/// moves between Running, Waiting and Idle every turn, so the reword must also leave focus alone, or it would move
/// under the user while they read. Specified: with the Delete question asking about running a and b and focus on its
/// Confirm, a listing that makes b idle with unseen output (the premise checked in the store) leaves the question open
/// with b's row saying "Idle — new output" in place of "Running", a's row still "Running", and focus still on Confirm.
#[gpui::test]
fn a_status_word_change_rewords_the_question_in_place(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(cx, fleet(&["a", "b"]));
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, DELETE_ID);
    test_support::settle(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-confirm"),
        "the premise"
    );
    assert!(
        status_words(cx, "b").contains(&"Running".to_owned()),
        "the premise: b's row says Running"
    );

    push(
        &mut fake,
        &["a", "b"],
        &[(
            session("b")
                .status(json!({"state": "idle"}))
                .activity(200)
                .seen(Some(100)),
            0,
        )],
        cx,
    );
    let live = cx.update(|_, cx| {
        let store = session::current_store(cx).expect("the premise: a UI session is live");
        let collections = store.read(cx).collections();
        collections.get::<SessionRow>("b").and_then(status::live)
    });
    assert_eq!(
        live,
        Some(Live::IdleUnseen),
        "the premise: b now reads \"Idle — new output\""
    );
    assert!(drawn(cx, "prompt"));
    assert_eq!(question_rows(cx), ["a", "b"]);
    let b = status_words(cx, "b");
    assert!(b.contains(&"Idle — new output".to_owned()), "{b:?}");
    assert!(!b.contains(&"Running".to_owned()), "{b:?}");
    assert!(status_words(cx, "a").contains(&"Running".to_owned()));
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
}

/// Why this matters: the bulk question can stay open while agents work, so a listing can reword it after the user has
/// moved focus inside it, onto Cancel to back out, say. Closing it and asking again, as this port once did, put focus
/// back on a starting answer each time; a reword in place must leave it on the control the user chose, and the answers
/// must then act on the reworded targets. Specified: with the Delete question over running a and b and Shift+Tab
/// pressed (focus on Cancel), b exiting (checked in the store) rewords b's row to "Delete anyway:" with focus still on
/// Cancel; Tab to Confirm and, once the new words are on screen, Enter sends a's Delete with no flag, then b's with
/// the reworded row's `only_if_nothing_alive`.
#[gpui::test]
fn a_reword_keeps_focus_where_the_user_put_it_in_the_question(cx: &mut TestAppContext) {
    let unmanaged = |key| session(key).unmanaged();
    let (mut fake, _list, cx) = open_list(cx, rows(vec![unmanaged("a"), unmanaged("b")]));
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, DELETE_ID);
    test_support::settle(cx);
    assert_eq!(question_rows(cx), ["a", "b"], "the premise");
    test_support::keys(cx, "shift-tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-cancel"),
        "the premise: the user moved focus to Cancel"
    );

    push(&mut fake, &["a", "b"], &[(ended("b").unmanaged(), 0)], cx);
    let b_ended = cx.update(|_, cx| {
        let store = session::current_store(cx).expect("the premise: a UI session is live");
        let collections = store.read(cx).collections();
        collections
            .get::<SessionRow>("b")
            .map(|row| row.info.status.has_ended())
    });
    assert_eq!(b_ended, Some(true), "the premise: b ended in the store");
    assert!(texts(cx).iter().any(|text| text == "Delete anyway:"));
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));

    test_support::keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-confirm"),
        "the premise"
    );
    show_next_frame(cx);
    test_support::keys(cx, "enter");
    let first = fake.commands();
    assert_eq!(
        owned(&lifecycle(first.clone()).0),
        [delete("/api/sessions/a")]
    );
    for command in &first {
        done(&mut fake, command);
    }
    test_support::settle(cx);
    assert_eq!(
        owned(&lifecycle(fake.commands()).0),
        [delete("/api/sessions/b?only_if_nothing_alive=true")],
        "b goes with the guard the reworded row drew"
    );
}

// ---- Retired containerized sessions ---------------------------------------------------------------------------------

/// Why this matters: SPEC.md, "Retired containerized sessions": such a session supports Delete only, and the helm
/// refuses its Stop, so a bulk Stop that sent one would only leave a refusal behind; the old bar left it out of Stop
/// without a word and kept its mark, since it was never the batch's; its Delete took every mark. Specified: with
/// ordinary o open and retired r and ordinary b marked, Stop asks nothing and sends b's Stop alone, r gets no line, and
/// once b's Stop is answered b's mark is gone and r's stays ("1 marked"); with b marked again, Delete asks about r and
/// b, and its Confirm sends r's Delete and then b's.
#[gpui::test]
fn a_bulk_stop_leaves_a_retired_session_out_and_delete_takes_it(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(cx, rows(vec![session("o"), retired("r"), session("b")]));
    requests(&mut fake);
    mark(cx, &["session-row-r", "session-row-b"]);
    assert_eq!(count(cx).as_deref(), Some("2 marked"), "the premise");

    test_support::click(cx, STOP_ID);
    test_support::settle(cx);
    assert!(!drawn(cx, "prompt"), "neither lists a tab");
    let sent = fake.commands();
    assert_eq!(owned(&sent), [stop("b")]);
    assert_eq!(label_of(cx, "session-outcome-r"), None);

    done(&mut fake, &sent[0]);
    test_support::settle(cx);
    assert_eq!(lifecycle(fake.commands()).0.len(), 0, "nothing for r");
    assert!(!stored(&list, "b", cx), "b's mark was the batch's");
    assert!(stored(&list, "r", cx), "r was never a target");
    assert_eq!(count(cx).as_deref(), Some("1 marked"));

    mark(cx, &["session-row-b"]);
    test_support::click(cx, DELETE_ID);
    test_support::settle(cx);
    assert_eq!(
        (drawn(cx, "bulk-target-r"), drawn(cx, "bulk-target-b")),
        (true, true),
        "the question asks about both"
    );
    test_support::click(cx, "prompt-confirm");
    test_support::settle(cx);
    let first = fake.commands();
    assert_eq!(
        owned(&lifecycle(first.clone()).0),
        [delete("/api/sessions/r")]
    );
    for command in &first {
        done(&mut fake, command);
    }
    test_support::settle(cx);
    assert_eq!(
        owned(&lifecycle(fake.commands()).0),
        [delete("/api/sessions/b")]
    );
}

/// Why this matters: SPEC.md, "Sandbox sessions": a session whose environment is being deleted has its actions
/// unavailable, and the old batch left such a target out with "Delete: <reason>" on its row while its siblings went on.
/// The ordinary not-started line ("another operation was in progress. Try again.") would send the user to retry what
/// cannot run until the environment is gone. Specified: with Skip on, a on a sandbox being deleted and ordinary b on
/// another host, both marked, Stop sends b's Stop alone and a's row says "Stop: This session's sandbox is being
/// deleted, so its actions are unavailable"; once b's Stop is answered, b's mark is gone and a's stays, since a was
/// never the batch's; with b marked again, Delete sends b's Delete alone and a's row says "Delete: " and the same
/// reason.
#[gpui::test]
fn a_bulk_action_leaves_out_a_session_being_deleted_and_says_why(cx: &mut TestAppContext) {
    let mut snapshot = snapshot(&[(session("a"), 0), (session("b").host(3), 0)], true);
    snapshot.push(registry(&environment_host(
        Provider::Sandbox,
        deleting(None),
    )));
    let (mut fake, list, cx) = open_list(cx, snapshot);
    requests(&mut fake);
    let reason = "This session's sandbox is being deleted, so its actions are unavailable";

    mark(cx, &["session-row-a", "session-row-b"]);
    assert_eq!(count(cx).as_deref(), Some("2 marked"), "the premise");
    test_support::click(cx, STOP_ID);
    test_support::settle(cx);
    let sent = fake.commands();
    assert_eq!(owned(&sent), [stop("b")]);
    assert_eq!(
        label_of(cx, "session-outcome-a"),
        Some(format!("Stop: {reason}"))
    );
    done(&mut fake, &sent[0]);
    test_support::settle(cx);
    assert!(!stored(&list, "b", cx), "b's mark was the batch's");
    assert!(stored(&list, "a", cx), "a was never a target");

    mark(cx, &["session-row-b"]);
    test_support::click(cx, DELETE_ID);
    test_support::settle(cx);
    assert_eq!(
        owned(&lifecycle(fake.commands()).0),
        [delete("/api/sessions/b")]
    );
    assert_eq!(
        label_of(cx, "session-outcome-a"),
        Some(format!("Delete: {reason}"))
    );
}

/// Why this matters: the bulk question's answer authorizes what it draws, and the old batch left a session whose
/// environment is being deleted out when the batch was drawn (`exclude_unavailable`), so the question never offers
/// what cannot run. Specified: with running a on a sandbox being deleted and running b on another host, both marked,
/// Delete leaves "Delete: This session's sandbox is being deleted, so its actions are unavailable" on a's row at once
/// and asks about b alone; Confirm sends b's Delete alone.
#[gpui::test]
fn the_bulk_question_leaves_out_a_session_being_deleted(cx: &mut TestAppContext) {
    let mut snapshot = snapshot(&[(session("a"), 0), (session("b").host(3), 0)], false);
    snapshot.push(registry(&environment_host(
        Provider::Sandbox,
        deleting(None),
    )));
    let (mut fake, _list, cx) = open_list(cx, snapshot);
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    test_support::click(cx, DELETE_ID);
    test_support::settle(cx);
    assert_eq!(question_rows(cx), ["b"], "a is left out of the question");
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some("Delete: This session's sandbox is being deleted, so its actions are unavailable")
    );
    test_support::click(cx, "prompt-confirm");
    test_support::settle(cx);
    assert_eq!(
        owned(&lifecycle(fake.commands()).0),
        [delete("/api/sessions/b")]
    );
}

/// Why this matters: a Stop that could act on none of the marks shown would be a control that does nothing; the old bar
/// drew Stop only while some mark was not a retired session, and kept Delete, which a retired session supports.
/// Specified: with retired r alone marked, the bar draws "1 marked" and Delete but no Stop; with ordinary b marked
/// too, it draws Stop again.
#[gpui::test]
fn with_only_retired_sessions_marked_the_bar_draws_delete_and_no_stop(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(cx, rows(vec![session("o"), retired("r"), session("b")]));
    requests(&mut fake);
    mark(cx, &["session-row-r"]);
    assert_eq!(count(cx).as_deref(), Some("1 marked"), "the premise");
    assert!(drawn(cx, DELETE_ID));
    assert!(!drawn(cx, STOP_ID));

    mark(cx, &["session-row-b"]);
    assert!(drawn(cx, STOP_ID));
}

/// Why this matters: the bulk question names the ⋯ as where its close returns focus; with only retired sessions
/// marked no metadata action applies and the bar draws no ⋯, so the question must name a control the bar does draw, or
/// Cancel would leave focus nowhere the user was. The old app fell back to the order's title, which the bar here
/// replaces. Specified: under the macOS click rule (a click focuses nothing, so the pressed Delete never held focus),
/// with retired r (running) alone marked, Delete asks about r, and Cancel sends nothing, keeps r marked and returns
/// focus to the bar's Delete.
#[gpui::test]
fn with_no_menu_toggle_cancel_returns_focus_to_the_pressed_button(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(cx, rows(vec![session("o"), retired("r")]));
    requests(&mut fake);
    mark(cx, &["session-row-r"]);
    assert!(!drawn(cx, MENU_TOGGLE_ID), "the premise: no ⋯");
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));

    test_support::click(cx, DELETE_ID);
    test_support::settle(cx);
    assert!(
        drawn(cx, "bulk-target-r"),
        "the premise: the question asks about r"
    );
    test_support::click(cx, "prompt-cancel");
    test_support::settle(cx);
    assert!(!drawn(cx, "prompt"));
    assert_eq!(lifecycle(fake.commands()).0.len(), 0);
    assert!(stored(&list, "r", cx));
    assert_eq!(focused_id(cx).as_deref(), Some(DELETE_ID));
}

//! Tests of a row's Stop and Replace: when each asks, the preconditions each request carries for what the row showed,
//! what a pending one shows and refuses, what a refusal or a success leaves on the row, and where a finished Replace
//! leaves the selection and focus.
//!
//! Everything runs in the real main view over the helm client's fake, as the menu's own tests do (whose fixtures and
//! helpers these share), and reads what the window sent from the fake and what it drew from GPUI's driver record of
//! the frame. A row's menu button is `session-menu-<key>`, its Replace and Stop are `session-menu-replace` and
//! `session-menu-stop`, a question's answers are `prompt-confirm` and `prompt-cancel`, and the lines under a row are
//! named `session-outcome-<key>` (a refusal or a notice) and `session-replace-progress-label-<key>` (a long Replace).

use std::time::Duration;

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use gpui::{AppContext as _, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::Root;
use serde_json::{Value, json};

use super::menu_tests::{any_layer, click_at, drawn, ended, entry, label_of, push, snapshot};
use super::outcome::PROGRESS_SHOWS_AFTER;
use super::tests::{change, listing_meta, open_main, painted, selected, sent, session};
use crate::focus::Mark;
use crate::main_view::MainView;
use crate::operations::OpKey;
use crate::test_support::{focused_id, frame, started};
use crate::{layers, session, test_support};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// Whether `command` is a Stop or a Replace.
fn is_lifecycle(command: &FakeCommand) -> bool {
    command.method == Method::Post
        && (command.path.ends_with("/stop") || command.path.ends_with("/replace"))
}

/// The Stops and Replaces the window sent since the last call, as (id, path, body), left unanswered for the test to
/// answer. Every other command is answered with success at once, as the helm answers a preference write or a seen mark;
/// the fake answers in order, so that works only while nothing sent before it waits, and a test with a Stop or Replace
/// waiting reads later commands with `sent` instead.
fn pending(fake: &mut FakeHelm) -> Vec<(u64, String, Value)> {
    let mut found = Vec::new();
    for command in fake.commands() {
        if is_lifecycle(&command) {
            found.push((command.id, command.path, command.body.unwrap_or_default()));
        } else {
            fake.reply(command.id, 200, None);
        }
    }
    found
}

/// [`pending`]'s requests without their ids.
fn requests(sent: Vec<(u64, String, Value)>) -> Vec<(String, Value)> {
    sent.into_iter()
        .map(|(_, path, body)| (path, body))
        .collect()
}

/// The Stops and Replaces the window sent since the last call, as (path, body), with every command answered in order:
/// each Stop and Replace as a success with an empty reply (which leaves nothing on its row and selects nothing), the
/// rest as for [`pending`]. For tests about what is sent, so that nothing waits behind a request they do not answer.
fn answered(fake: &mut FakeHelm) -> Vec<(String, Value)> {
    let mut found = Vec::new();
    for command in fake.commands() {
        if is_lifecycle(&command) {
            fake.reply(command.id, 200, Some(json!({})));
            found.push((command.path, command.body.unwrap_or_default()));
        } else {
            fake.reply(command.id, 200, None);
        }
    }
    found
}

/// A Stop of `key` with these preconditions, as the window sends it.
fn stop(key: &str, refuse_if_tabs_open: bool, refuse_if_agent_running: bool) -> (String, Value) {
    (
        format!("/api/sessions/{key}/stop"),
        json!({
            "refuse_if_tabs_open": refuse_if_tabs_open,
            "refuse_if_agent_running": refuse_if_agent_running,
        }),
    )
}

/// A plain Replace of `key` with these flags of its source's Delete guard, as the window sends it: nothing else, no
/// retry key and no YOLO consent.
fn replace(key: &str, only_if_nothing_alive: bool, only_if_agent_ended: bool) -> (String, Value) {
    (
        format!("/api/sessions/{key}/replace"),
        json!({
            "only_if_nothing_alive": only_if_nothing_alive,
            "only_if_agent_ended": only_if_agent_ended,
        }),
    )
}

/// Open the menu of the row whose menu button is `button` and choose `item`, by clicks.
fn choose(cx: &mut VisualTestContext, button: &'static str, item: &'static str) {
    test_support::click(cx, button);
    test_support::click(cx, item);
    test_support::settle(cx);
}

/// Whether the focused element is a session's terminal (the pane mounts one, the selected session's).
fn terminal_focused(cx: &mut VisualTestContext) -> bool {
    focused_id(cx).as_deref() == Some("terminal-grid")
}

/// The hover help of the open question's Cancel, as shown once the pointer has rested on it for the hover delay.
fn cancel_help(cx: &mut VisualTestContext) -> Option<String> {
    let cancel = test_support::center(cx, "prompt-cancel");
    cx.simulate_mouse_move(cancel, None, Modifiers::none());
    test_support::advance(cx, layers::HOVER_DELAY);
    cx.update(|window, cx| layers::shown_tooltip(window, cx))
        .map(|shown| shown.text.to_string())
}

/// Every line of text painted in a fresh frame.
fn texts(cx: &mut VisualTestContext) -> Vec<String> {
    painted(&frame(cx))
}

// ---- When each asks, and what each sends ---------------------------------------------------------------------------

/// SPEC.md, "Lifecycle operations": Stop asks only "With tabs open", and the old row never asked just because the agent
/// was alive; and the request carries what the click showed, so that the supervisor refuses a tab the user did not see
/// or an agent another client restarted ("Confirmed 2026-10-01"). Specified: a running row and an ended row without
/// tabs stop on the click, asking nothing; a running row with one tab asks "Stop the agent and close 1 terminal tab?"
/// with its title quoted, focus on Confirm and Cancel's help "Cancel: keep the agent and terminal tabs running" (the
/// old row's); an ended row with two tabs asks "Close 2 terminal tabs?"; the four Stops carry, in order, no tabs shown
/// and a live agent, no tabs and an ended agent, tabs and a live agent, and tabs and an ended agent.
#[gpui::test]
fn stop_asks_only_about_tabs_and_carries_what_its_row_showed(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(
        cx,
        snapshot(
            &[
                (session("a"), 0),
                (ended("b"), 0),
                (session("c"), 1),
                (ended("d"), 2),
            ],
            false,
        ),
    );
    pending(&mut fake);

    choose(cx, "session-menu-a", "session-menu-stop");
    assert!(!any_layer(cx), "a live agent without tabs asks nothing");
    choose(cx, "session-menu-b", "session-menu-stop");
    assert!(!any_layer(cx), "an ended agent without tabs asks nothing");

    choose(cx, "session-menu-c", "session-menu-stop");
    assert!(drawn(cx, "prompt"), "a row with a tab asks");
    assert!(
        texts(cx)
            .iter()
            .any(|text| text == "Stop the agent and close 1 terminal tab?")
    );
    assert_eq!(
        label_of(cx, "session-stop-title-c").as_deref(),
        Some("\"c\"")
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    assert_eq!(
        cancel_help(cx).as_deref(),
        Some("Cancel: keep the agent and terminal tabs running"),
        "Cancel's help is the old row question's"
    );
    test_support::click(cx, "prompt-confirm");

    choose(cx, "session-menu-d", "session-menu-stop");
    assert!(
        texts(cx)
            .iter()
            .any(|text| text == "Close 2 terminal tabs?")
    );
    test_support::click(cx, "prompt-confirm");

    assert_eq!(
        answered(&mut fake),
        [
            stop("a", true, false),
            stop("b", true, true),
            stop("c", false, false),
            stop("d", false, true),
        ]
    );
}

/// SPEC.md: "Replace keeps its confirmation whatever the agent is doing", unlike Delete, which goes ahead on a row with
/// nothing alive; and the source's removal carries Delete's precondition for what the question showed. Cancel must send
/// nothing. Specified: Replace on a running row asks with the old consequence and the quoted title, focus on Confirm
/// and Cancel's help "Cancel: keep this session as it is" (the old row's), and Cancel sends nothing; asked again and
/// confirmed, and then on an ended row with a tab and an ended row without one (each asking too), the three Replaces
/// carry no flag, `only_if_agent_ended`, and `only_if_nothing_alive`.
#[gpui::test]
fn replace_always_asks_and_carries_the_delete_guard_of_its_row(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(
        cx,
        snapshot(
            &[(session("a"), 0), (ended("b"), 1), (ended("c"), 0)],
            false,
        ),
    );
    pending(&mut fake);

    choose(cx, "session-menu-a", "session-menu-replace");
    assert!(drawn(cx, "prompt"));
    assert!(texts(cx).iter().any(|text| text
        == "Still running — replacing kills the agent and discards the conversation; a fresh session with the same \
            settings takes its place:"));
    assert_eq!(
        label_of(cx, "session-replace-title-a").as_deref(),
        Some("\"a\"")
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    assert_eq!(
        cancel_help(cx).as_deref(),
        Some("Cancel: keep this session as it is"),
        "Cancel's help is the old row question's"
    );
    test_support::click(cx, "prompt-cancel");
    assert!(!any_layer(cx));
    assert_eq!(pending(&mut fake), [], "Cancel sends nothing");

    choose(cx, "session-menu-a", "session-menu-replace");
    test_support::click(cx, "prompt-confirm");
    choose(cx, "session-menu-b", "session-menu-replace");
    assert!(drawn(cx, "prompt"), "an ended row with a tab asks");
    test_support::click(cx, "prompt-confirm");
    choose(cx, "session-menu-c", "session-menu-replace");
    assert!(drawn(cx, "prompt"), "so does a row with nothing alive");
    test_support::click(cx, "prompt-confirm");

    assert_eq!(
        answered(&mut fake),
        [
            replace("a", false, false),
            replace("b", false, true),
            replace("c", true, false),
        ]
    );
}

/// SPEC.md: with **Skip session confirmations** on, row Stop and row Replace each run "on the first click through the
/// ordinary action path, with consent from the state shown by that render", under the same guards. Specified: with the
/// preference on, Stop on a running row with a tab and Replace on an ended row with a tab open no question, and send a
/// Stop authorizing the tabs and a Replace guarded by the ended agent.
#[gpui::test]
fn skipped_confirmations_stop_and_replace_on_the_click_with_their_rows_guards(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 1), (ended("b"), 1)], true));
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-stop");
    assert!(!any_layer(cx));
    choose(cx, "session-menu-b", "session-menu-replace");
    assert!(!any_layer(cx));
    assert_eq!(
        answered(&mut fake),
        [stop("a", false, false), replace("b", false, true)]
    );
}

/// The prompt's guarantee at the real Stop and Replace questions (SPEC_impl.md, "Questions: the prompt"): whatever
/// runs acts on the state the question froze when it opened, even after the session changed under it, and only the
/// first of two answers delivered with no redraw between them runs. Each question here is opened on a running row,
/// then the helm pushes that row with its agent ended and no tabs, which would have given other preconditions.
/// Specified: Stop's Cancel then Confirm sends nothing; Stop's Confirm then Cancel sends the Stop of the row as asked
/// (tabs shown, a live agent); the same two orders on Replace send nothing, then the Replace with no flag.
#[gpui::test]
fn an_answer_sends_the_preconditions_its_question_was_drawn_from(cx: &mut TestAppContext) {
    let rows = [
        (session("a"), 1),
        (session("b"), 1),
        (session("c"), 0),
        (session("d"), 0),
    ];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    pending(&mut fake);

    for (key, button, item, order, expected) in [
        (
            "a",
            "session-menu-a",
            "session-menu-stop",
            ["prompt-cancel", "prompt-confirm"],
            vec![],
        ),
        (
            "b",
            "session-menu-b",
            "session-menu-stop",
            ["prompt-confirm", "prompt-cancel"],
            vec![stop("b", false, false)],
        ),
        (
            "c",
            "session-menu-c",
            "session-menu-replace",
            ["prompt-cancel", "prompt-confirm"],
            vec![],
        ),
        (
            "d",
            "session-menu-d",
            "session-menu-replace",
            ["prompt-confirm", "prompt-cancel"],
            vec![replace("d", false, false)],
        ),
    ] {
        choose(cx, button, item);
        assert!(any_layer(cx), "the premise: {key}'s question is open");
        push(&mut fake, &["a", "b", "c", "d"], &[(ended(key), 0)], cx);
        assert!(any_layer(cx), "{key}: an ended session is not gone");
        let points = [
            test_support::center(cx, order[0]),
            test_support::center(cx, order[1]),
        ];
        cx.update(|window, cx| {
            for at in points {
                for event in click_at(at) {
                    window.dispatch_event(event, cx);
                }
            }
        });
        test_support::settle(cx);
        assert_eq!(answered(&mut fake), expected, "{key}: {order:?}");
        assert!(!any_layer(cx), "{key}: the question closed");
    }
}

// ---- While a Stop or a Replace runs --------------------------------------------------------------------------------

/// SPEC.md: "A Stop shows a spinner and **Stopping…** on its row", and "Each session excludes its own competing
/// actions … Opening the session remains possible" while other sessions act normally. Unlike Delete's, a Stop's row
/// keeps its title, since the session stays. Specified: with a's Stop sent and unanswered, a's row paints "Stopping…"
/// and its title; in a's menu, Rename, Replace, Stop and Delete each ignore a click (the menu stays open); b's Replace
/// still asks; a click on a's row opens a; nothing but the opening's preference write and seen mark is sent. Once the
/// Stop is answered, "Stopping…" is gone and a's Replace asks again.
#[gpui::test]
fn a_pending_stop_shows_beside_the_title_and_refuses_only_its_own_sessions_actions(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("b"), 0), (session("a"), 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-stop");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Stop was sent");
    };

    let painted = texts(cx);
    assert!(painted.iter().any(|text| text == "Stopping…"));
    assert!(painted.iter().any(|text| text == "a"), "the title stays");
    test_support::click(cx, "session-menu-a");
    for item in [
        "session-menu-rename",
        "session-menu-replace",
        "session-menu-stop",
        "session-menu-delete",
    ] {
        test_support::click(cx, item);
        assert!(
            drawn(cx, "session-menu"),
            "{item} is unavailable: the menu is still open"
        );
        assert!(!drawn(cx, "prompt"), "{item} asks nothing");
    }
    test_support::keys(cx, "escape");

    choose(cx, "session-menu-b", "session-menu-replace");
    assert!(drawn(cx, "prompt"), "b's Replace still asks");
    test_support::click(cx, "prompt-cancel");
    test_support::click(cx, "session-row-a");
    assert_eq!(selected(cx).as_deref(), Some("a"));
    // The opening's writes wait behind a's Stop, so they are read, not answered.
    for (path, _) in sent(&mut fake) {
        assert!(
            path == "/api/preferences" || path == "/api/sessions/a/seen",
            "only the opening's writes were sent: {path}"
        );
    }

    fake.reply(id, 200, Some(json!({})));
    test_support::settle(cx);
    assert!(!texts(cx).iter().any(|text| text == "Stopping…"));
    choose(cx, "session-menu-a", "session-menu-replace");
    assert!(
        drawn(cx, "prompt"),
        "a's Replace asks once its Stop settled"
    );
}

/// SPEC.md: "After about two seconds waiting for the reply, the sidebar … share a spinner, the host and elapsed
/// seconds, updated about once a second. The status disappears on the reply." The old `LaunchProgress` showed nothing
/// before two whole seconds, so a quick reply adds no height to the row. Time is the executor's clock, moved by the
/// test. Specified: with a's Replace confirmed and unanswered, nothing shows 1.9 seconds in; at two seconds the row says
/// "Replacing on buildbox…" (the row's host, as the helm names it) and "2 s", then "3 s" and "4 s" a second apart, while
/// a's Stop in its menu is unavailable; once the helm refuses, the line is gone and the refusal shows instead.
#[gpui::test]
fn a_replace_says_after_two_seconds_how_long_it_has_waited(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0)], false));
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-replace");
    test_support::click(cx, "prompt-confirm");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Replace was sent");
    };
    let progress = |cx: &mut VisualTestContext| label_of(cx, "session-replace-progress-label-a");

    test_support::advance(cx, PROGRESS_SHOWS_AFTER - Duration::from_millis(100));
    assert_eq!(progress(cx), None, "nothing before two seconds");
    test_support::advance(cx, Duration::from_millis(100));
    assert_eq!(progress(cx).as_deref(), Some("Replacing on buildbox…"));
    assert!(texts(cx).iter().any(|text| text == "2 s"));
    for seconds in ["3 s", "4 s"] {
        test_support::advance(cx, Duration::from_secs(1));
        assert!(texts(cx).iter().any(|text| text == seconds), "{seconds}");
    }
    test_support::click(cx, "session-menu-a");
    test_support::click(cx, "session-menu-stop");
    assert!(
        drawn(cx, "session-menu"),
        "a's Stop is unavailable meanwhile"
    );
    test_support::keys(cx, "escape");

    fake.reply(id, 409, Some(json!("the host refused this operation")));
    test_support::settle(cx);
    assert_eq!(progress(cx), None, "the line goes with the reply");
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some("Replace: the host refused this operation")
    );
}

/// SPEC.md, "Session list": "Pending Stop batches do not delay automatic selection", and only Deletes hold it back; a
/// selection that went away while a Stop or Replace of another session runs must not wait for them. Specified: with b
/// open and a Stop of a and a Replace of c in flight, a pushed complete listing without b opens a, the first of the
/// newest (b was the only visit). Both requests are left unanswered, and the store is asked that they are still in
/// flight, since a test whose requests had settled would pass whatever pending work did to the choice.
#[gpui::test]
fn a_pending_stop_or_replace_holds_no_automatic_choice_back(cx: &mut TestAppContext) {
    let rows = [(session("b"), 0), (session("a"), 0), (session("c"), 0)];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, true));
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-stop");
    choose(cx, "session-menu-c", "session-menu-replace");
    assert_eq!(
        requests(pending(&mut fake)),
        [stop("a", true, false), replace("c", false, false)],
    );
    assert!(
        cx.update(|_, cx| super::stop::in_flight("a", cx) && super::replace::in_flight("c", cx)),
        "the premise: both are in flight"
    );

    fake.push(change(listing_meta(&["a", "c"], 2, false), &[], &["b"]));
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("a"));
}

// ---- What a settled Stop or Replace leaves -------------------------------------------------------------------------

/// SPEC.md: "A failure or partial result appears on the source session's row", and the old app kept it there, named by
/// its operation ("Stop: …", "Replace: …"), until the next operation on the session started; the open menu covers the
/// row's copy, so the menu shows it too. Without the YOLO question (the launcher's), a host's YOLO refusal of a Replace
/// shows the same way. Specified: a refused Stop of a shows "Stop: <the helm's words>" on a's row and in a's menu,
/// stays through an unrelated pushed change, and goes once a's next Stop is sent; a Replace of b refused with the
/// helm's YOLO words shows them after "Replace: ".
#[gpui::test]
fn a_refusal_shows_on_the_row_and_in_its_menu_until_the_next_operation(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-stop");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Stop was sent");
    };
    fake.reply(id, 409, Some(json!("the agent is busy")));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some("Stop: the agent is busy")
    );
    test_support::click(cx, "session-menu-a");
    assert_eq!(
        label_of(cx, "session-menu-outcome-a").as_deref(),
        Some("Stop: the agent is busy")
    );
    test_support::keys(cx, "escape");
    push(
        &mut fake,
        &["a", "b"],
        &[(session("b").title("renamed"), 0)],
        cx,
    );
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some("Stop: the agent is busy")
    );

    choose(cx, "session-menu-a", "session-menu-stop");
    let [(again, _, _)] = pending(&mut fake)[..] else {
        panic!("a's next Stop was sent");
    };
    assert_eq!(
        label_of(cx, "session-outcome-a"),
        None,
        "the refusal goes when the next Stop starts, not when it settles"
    );

    let yolo = "buildbox asks before YOLO launches, and this launch is YOLO or uses the agent's own setting: nothing \
                was started.";
    choose(cx, "session-menu-b", "session-menu-replace");
    test_support::click(cx, "prompt-confirm");
    let [(replaced, _, _)] = pending(&mut fake)[..] else {
        panic!("b's Replace was sent");
    };
    fake.reply(again, 200, Some(json!({})));
    fake.reply(replaced, 409, Some(json!(yolo)));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-outcome-b").as_deref(),
        Some(format!("Replace: {yolo}").as_str())
    );
}

/// SPEC.md, "Lifecycle operations": "A tab that cannot be closed does not prevent the other tabs from closing; the reply
/// names the failed tabs and the client shows that notice", which the old app put in the row's one slot as "Stop: …"
/// until the next operation. The completion notes it in the operation store's slot for the session, which must give it
/// up when any next operation of the session starts, not only a Stop. Specified: a Stop answered with a notice shows
/// "Stop: <notice>" on the row; it stays through a pushed change; a Rename of the session started through the operation
/// store takes it away, and it does not come back when that Rename succeeds.
#[gpui::test]
fn a_stops_notice_stays_on_its_row_until_the_next_operation_starts(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-stop");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Stop was sent");
    };
    let notice = "Terminal 2 could not be closed: host unavailable";
    fake.reply(id, 200, Some(json!({ "notice": notice })));
    test_support::settle(cx);
    let shown = format!("Stop: {notice}");
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some(shown.as_str())
    );
    push(&mut fake, &["a", "b"], &[(session("a").title("a"), 0)], cx);
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some(shown.as_str())
    );

    cx.update(|_, cx| {
        let operations = session::current_operations(cx).unwrap();
        let rename = super::rename::command("a", "renamed").unwrap();
        operations.update(cx, |operations, cx| {
            operations
                .start(&OpKey::session("a"), rename, cx, |_, _| {})
                .unwrap()
        });
    });
    test_support::settle(cx);
    assert_eq!(label_of(cx, "session-outcome-a"), None);
    let [rename] = &fake.commands()[..] else {
        panic!("one Rename was sent");
    };
    fake.reply(rename.id, 200, None);
    test_support::settle(cx);
    assert_eq!(label_of(cx, "session-outcome-a"), None);
}

/// a and b with a open, and a's Replace asked from its row, confirmed and sent: the window, the fake, and the Replace's
/// id for the test to answer.
fn replacing_the_open_session(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext, u64) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-replace");
    test_support::click(cx, "prompt-confirm");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Replace was sent");
    };
    (fake, cx, id)
}

/// The helm's reply to a's Replace once it has made n: n's own session, as the helm's fixture shapes one, and the notice
/// the source's Delete carried.
fn replacement_reply() -> Value {
    let mut reply = session("n").entry().value;
    reply["delete_notice"] = json!("the checkout moved to the trash");
    reply
}

/// What the helm does once a's Replace has made n: it pushes a gone and n listed, then answers with n's session and the
/// notice the source's Delete carried. Between the two, the selection must not have moved for the removal: `selected`
/// is what was open just before.
fn the_helm_replaces_a_with_n(
    fake: &mut FakeHelm,
    cx: &mut VisualTestContext,
    id: u64,
    selected_before: &str,
) {
    fake.push(change(
        listing_meta(&["n", "b"], 2, false),
        &[session("n")],
        &["a"],
    ));
    test_support::settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some(selected_before),
        "the source's removal moves no selection while its Replace runs"
    );
    fake.reply(id, 200, Some(replacement_reply()));
    test_support::settle(cx);
}

/// SPEC.md: "A completed Replace opens its replacement only if the source is still selected", and "If the user has not
/// moved on, the result may take focus" ("Who may move focus"): the user sat and waited for this one. The reply must
/// move focus itself: the replacement's terminal only becoming home would leave focus on whatever control held it. So a
/// control holds focus when the reply lands, with no input since Confirm: b's menu button, focused before the question
/// was asked (its menu opened and closed), which is where closing the question returns focus. The question is asked
/// through the list's own Replace, as the menu's item asks it, so that its opener is that button. The reply's
/// `delete_notice` has no list band to go to yet and the source's row is gone, so it shows on the replacement's row.
/// Specified: focus on b's button, a's Replace asked and confirmed with Enter, focus back on b's button; the helm
/// removes a and lists n; the reply opens n, whose terminal takes focus, and n's row shows "Replace: <notice>".
#[gpui::test]
fn a_replace_of_the_open_session_opens_its_replacement_and_focuses_it(cx: &mut TestAppContext) {
    replace_the_open_session_from_bs_button(cx, Confirm::Enter);
}

/// The same as [`a_replace_of_the_open_session_opens_its_replacement_and_focuses_it`] with Confirm clicked. The
/// question's card sits partly over the terminal, and a click on Confirm once also reached the terminal under it,
/// which took focus, so a click left focus on the terminal where Enter left it on the question's opener. Specified: the
/// same, with Confirm pressed by a mouse click.
#[gpui::test]
fn a_replace_confirmed_with_a_click_returns_focus_like_enter(cx: &mut TestAppContext) {
    replace_the_open_session_from_bs_button(cx, Confirm::Click);
}

/// How a test presses a question's Confirm.
#[derive(Clone, Copy)]
enum Confirm {
    Enter,
    Click,
}

/// The two focus tests above: see the first's docs.
fn replace_the_open_session_from_bs_button(cx: &mut TestAppContext, confirm: Confirm) {
    let (mut fake, _connection) = started(cx);
    fake.connect(snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    cx.run_until_parked();
    let (root, cx) = cx.add_window_view(|window, cx| {
        let main = cx.new(|cx| MainView::new(window, cx));
        Root::new(main, window, cx)
    });
    cx.update(|window, _| window.activate_window());
    test_support::settle(cx);
    let list = cx.update(|_, cx| {
        let main = root.read(cx).view().clone().downcast::<MainView>().unwrap();
        main.read(cx).session_list().clone()
    });
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    pending(&mut fake);
    test_support::click(cx, "session-menu-b");
    test_support::keys(cx, "escape");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-b"),
        "the premise: focus is on b's button"
    );

    cx.update(|window, cx| {
        let mark = Mark::take(cx);
        list.update(cx, |list, cx| list.replace("a", &mark, window, cx));
    });
    test_support::settle(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-confirm"),
        "the premise: the question starts on Confirm"
    );
    match confirm {
        Confirm::Enter => test_support::keys(cx, "enter"),
        Confirm::Click => test_support::click(cx, "prompt-confirm"),
    }
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Replace was sent");
    };
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-b"),
        "the premise: the question gave focus back to b's button, and nothing was touched since Confirm"
    );
    the_helm_replaces_a_with_n(&mut fake, cx, id, "a");
    assert_eq!(selected(cx).as_deref(), Some("n"));
    assert!(
        terminal_focused(cx),
        "the replacement's terminal took focus from b's button"
    );
    assert_eq!(
        label_of(cx, "session-outcome-n").as_deref(),
        Some("Replace: the checkout moved to the trash")
    );
}

/// The hold on a Replace's source ends with the Replace however it ends. A Replace can be refused after the helm already
/// removed its source (the helm's "whether the original … was removed is unknown"), and then the list must move on as
/// for any removal rather than keep a selection that is gone. Those words belong to the source, whose row is gone, so
/// nothing shows them (recorded in the parity file's deviations). Specified: with a's Replace sent, a pushed removal of
/// a with n listed keeps a selected; a refusal then moves the selection off a, to n, and the refusal's words are
/// painted nowhere.
#[gpui::test]
fn a_replace_refused_after_its_source_went_lets_the_selection_move_on(cx: &mut TestAppContext) {
    let (mut fake, cx, id) = replacing_the_open_session(cx);
    fake.push(change(
        listing_meta(&["n", "b"], 2, false),
        &[session("n")],
        &["a"],
    ));
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is held");
    let words = "created replacement session n but whether the original a was removed is unknown after a lost reply; \
                 the replacement exists — check a before deleting it or retrying";
    fake.reply(id, 500, Some(json!(words)));
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("n"));
    assert!(
        !texts(cx)
            .iter()
            .any(|text| text.contains("whether the original")),
        "the source's refusal has no row to show on"
    );
}

/// A Replace's `delete_notice` goes on the replacement's row, a key other than the Replace's own, where newer work can
/// already run: the helm lists the replacement before it removes the source and answers, so the user can Stop it
/// meanwhile. A note under that Stop would outlive it (a success does not clear the slot), breaking "shown until the
/// next operation starts", so the notice is dropped. Specified: with a's Replace sent and n listed without tabs, n's
/// Stop is sent; the Replace's reply carries a delete notice; once n's Stop succeeds, n's row shows no line.
#[gpui::test]
fn a_delete_notice_is_dropped_when_the_replacement_already_has_work_running(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx, id) = replacing_the_open_session(cx);
    let mut listed = change(listing_meta(&["n", "b"], 2, false), &[], &["a"]);
    listed.upsert = vec![entry(&session("n"), 0)];
    fake.push(listed);
    test_support::settle(cx);
    choose(cx, "session-menu-n", "session-menu-stop");
    let [(stopped, _, _)] = pending(&mut fake)[..] else {
        panic!("n's Stop was sent");
    };
    fake.reply(id, 200, Some(replacement_reply()));
    test_support::settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some("n"),
        "the premise: the reply was read and opened n (a was open and held, so only a decoded reply moves it)"
    );
    fake.reply(stopped, 200, Some(json!({})));
    test_support::settle(cx);
    assert_eq!(label_of(cx, "session-outcome-n"), None);
}

/// SPEC.md, "Who may move focus": "Once the user has moved on, the result moves no focus". The replacement still opens,
/// since its source is still the open session, but focus stays where the user put it. Focus is put on a control (b's
/// menu button) so the difference shows: a terminal that took focus away from a control would be the result moving it.
/// Specified: with a's Replace sent, the user opens b's menu and closes it with Escape, leaving focus on b's menu
/// button; the reply opens n, and focus is still on b's button.
#[gpui::test]
fn a_replace_after_the_user_moved_on_opens_its_replacement_without_focus(cx: &mut TestAppContext) {
    let (mut fake, cx, id) = replacing_the_open_session(cx);
    test_support::click(cx, "session-menu-b");
    test_support::keys(cx, "escape");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-b"),
        "the premise: focus is on b's button"
    );
    the_helm_replaces_a_with_n(&mut fake, cx, id, "a");
    assert_eq!(selected(cx).as_deref(), Some("n"));
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-b"));
}

/// SPEC.md: a completed Replace whose source is no longer selected "appears in the list and the user stays where they
/// are". Specified: with a's Replace sent, the user opens b; the pushed removal and the reply leave b open, with n
/// listed.
#[gpui::test]
fn a_replace_after_the_user_opened_another_session_leaves_them_there(cx: &mut TestAppContext) {
    let (mut fake, cx, id) = replacing_the_open_session(cx);
    test_support::click(cx, "session-row-b");
    the_helm_replaces_a_with_n(&mut fake, cx, id, "b");
    assert_eq!(selected(cx).as_deref(), Some("b"));
    assert_eq!(label_of(cx, "session-row-n").as_deref(), Some("n"));
}

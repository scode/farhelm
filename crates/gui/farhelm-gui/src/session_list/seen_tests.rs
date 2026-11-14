//! Tests of a row's manual Mark read and Mark unread, and of the open session's seen memory: the menu item and the
//! status dot, the exact body each sends, what a pending or refused toggle does to the row, and that a manual Mark
//! unread of the open session survives everything but the user leaving and coming back, or new output, including a
//! listing that hides the session for a while.
//!
//! Everything runs in the real main view over the helm client's fake, as the menu's own tests do (whose fixtures and
//! helpers these share), and reads what the window sent from the fake and what it drew from GPUI's driver record of
//! the frame. A row's menu button is `session-menu-<key>`, the menu's toggle is `session-menu-seen`, a row's status dot
//! is `session-seen-<key>`, and the line under a row is `session-outcome-<key>`.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{CollectionMeta, ListCounts, SESSION_QUERY_PATH};
use gpui::{Modifiers, TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::host_rail::tests::{hosts, local, remote};
use super::menu_tests::{any_layer, drawn, ended, label_of, push, snapshot};
use super::tests::{Fixture, change, listing_meta, open_main, selected, session};
use crate::actions::QUICK_SWITCHER_CHORD;
use crate::test_support::{focused_id, meta};
use crate::{layers, test_support};

// ---- Fixtures and helpers ------------------------------------------------------------------------------------------

/// An idle session titled by its key, active and seen at 100, as [`session`] makes it: it reads seen.
fn idle(key: &'static str) -> Fixture {
    session(key).status(json!({"state": "idle"}))
}

/// [`idle`] with output nobody has looked at: active at 250, seen at 100.
fn unseen(key: &'static str) -> Fixture {
    idle(key).activity(250).seen(Some(100))
}

/// Every command the window sent since the last call, as (method, path, body), each answered with success in order, as
/// the helm answers a seen write, a preference write or a session query. For tests about what is sent, so that nothing
/// waits behind a command they do not answer.
fn answered(fake: &mut FakeHelm) -> Vec<(Method, String, Value)> {
    fake.commands()
        .into_iter()
        .map(|command| {
            fake.reply(command.id, 200, None);
            (
                command.method,
                command.path,
                command.body.unwrap_or_default(),
            )
        })
        .collect()
}

/// The manual toggle or the automatic mark of `key`, as the window sends either: a PUT of `seen_activity_at`.
fn seen_put(key: &str, seen_activity_at: Value) -> (Method, String, Value) {
    (
        Method::Put,
        format!("/api/sessions/{key}/seen"),
        json!({ "seen_activity_at": seen_activity_at }),
    )
}

/// Opening `key` as the window tells the helm: its `last_selected` preference.
fn remembered(key: &str) -> (Method, String, Value) {
    (
        Method::Put,
        "/api/preferences".to_owned(),
        json!({ "last_selected": key }),
    )
}

/// Open the menu of `key`'s row and choose its Mark read or Mark unread, by clicks.
fn toggle_from_menu(cx: &mut VisualTestContext, button: &'static str) {
    test_support::click(cx, button);
    test_support::click(cx, "session-menu-seen");
    test_support::settle(cx);
}

/// The hover help showing once the pointer has crossed into the control drawn under `selector` from the middle of the
/// row around it (`row`), as a pointer reaches a row's dot, and rested for the hover delay.
fn help_after_crossing(
    cx: &mut VisualTestContext,
    row: &'static str,
    selector: &'static str,
) -> Option<String> {
    for at in [
        test_support::center(cx, row),
        test_support::center(cx, selector),
    ] {
        cx.simulate_mouse_move(at, None, Modifiers::none());
        test_support::settle(cx);
    }
    test_support::advance(cx, layers::HOVER_DELAY);
    cx.update(|window, cx| layers::shown_tooltip(window, cx))
        .map(|shown| shown.text.to_string())
}

// ---- The menu item -------------------------------------------------------------------------------------------------

/// SPEC.md, "Session list" and "Status": a row's menu offers a Mark read / Mark unread toggle that "sets or clears the
/// stamp directly", and the old menu's label followed the row's current seen state on every render. The body is the
/// route's contract: a stamp to record, or `null` to clear it (a missing key would be refused, never read as unread).
/// The old `sidebar.spec.ts` case "the mark-seen item is reachable and operable by role and keyboard" is the first half.
/// Specified: with a open and b idle with unseen output, b's menu offers "Mark read" right after Rename (Tab from Rename
/// reaches it) with its description; Enter on it closes the menu, leaves focus on b's button and sends
/// `PUT /api/sessions/b/seen` with b's latest activity; once the helm pushes b seen, b's menu offers "Mark unread", relabels itself "Mark read" when new
/// output arrives while it is open and "Mark unread" again once that is seen, and choosing it sends `null`; a stays
/// open throughout; an ended row's menu offers neither.
#[gpui::test]
fn the_menu_item_follows_the_seen_state_and_sends_the_stamp_or_null(cx: &mut TestAppContext) {
    let rows = [(session("a"), 0), (unseen("b"), 0), (ended("c"), 0)];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    answered(&mut fake);

    test_support::click(cx, "session-menu-b");
    assert_eq!(
        label_of(cx, "session-menu-seen").as_deref(),
        Some("Mark read")
    );
    test_support::keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-seen"),
        "the item comes right after Rename"
    );
    assert!(
        super::tests::painted(&test_support::frame(cx))
            .iter()
            .any(|text| text == "Treat the latest output as seen")
    );
    test_support::keys(cx, "enter");
    test_support::settle(cx);
    assert!(!any_layer(cx), "the menu closed");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-b"));
    assert_eq!(answered(&mut fake), [seen_put("b", json!(250))]);

    let order = ["a", "b", "c"];
    push(&mut fake, &order, &[(unseen("b").seen(Some(250)), 0)], cx);
    test_support::click(cx, "session-menu-b");
    assert_eq!(
        label_of(cx, "session-menu-seen").as_deref(),
        Some("Mark unread")
    );
    push(
        &mut fake,
        &order,
        &[(unseen("b").activity(300).seen(Some(250)), 0)],
        cx,
    );
    assert_eq!(
        label_of(cx, "session-menu-seen").as_deref(),
        Some("Mark read"),
        "the open menu follows new output"
    );
    push(
        &mut fake,
        &order,
        &[(unseen("b").activity(300).seen(Some(300)), 0)],
        cx,
    );
    assert_eq!(
        label_of(cx, "session-menu-seen").as_deref(),
        Some("Mark unread")
    );
    test_support::click(cx, "session-menu-seen");
    test_support::settle(cx);
    assert_eq!(answered(&mut fake), [seen_put("b", Value::Null)]);
    assert_eq!(selected(cx).as_deref(), Some("a"));

    test_support::click(cx, "session-menu-c");
    assert!(
        !drawn(cx, "session-menu-seen"),
        "an ended row offers no toggle"
    );
}

/// SPEC.md, "Errors and diagnostics": unlike the automatic mark, "a failed toggle … surfaces like any other operation",
/// on the session's row, and the old app kept it there as "Seen: <the helm's words>" until the row's next operation.
/// Its menu closed at acceptance, and the refusal must not close or take focus from a menu the user opened meanwhile
/// (the old `sidebar.spec.ts` case "a failed manual mark closes on acceptance and preserves a newer menu"). Specified:
/// b's toggle closes b's menu at once; with it unanswered, a's menu is opened; a refusal with the helm's words leaves
/// a's menu open with focus on its Rename, and shows "Seen: <words>" under b's row and in b's menu; it stays through an
/// unrelated pushed change, and goes once b's next toggle is sent, before it is answered.
#[gpui::test]
fn a_refused_toggle_shows_on_the_row_until_the_next_operation(cx: &mut TestAppContext) {
    let rows = [(session("a"), 0), (unseen("b"), 0)];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    answered(&mut fake);
    toggle_from_menu(cx, "session-menu-b");
    assert!(!any_layer(cx), "b's menu closed at acceptance");
    let [refused] = &fake.commands()[..] else {
        panic!("one toggle was sent");
    };
    let refused = refused.id;
    test_support::click(cx, "session-menu-a");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-rename"),
        "the premise: a's menu is open on its Rename"
    );
    fake.reply(refused, 500, Some(json!("seen-failure-sentinel")));
    test_support::settle(cx);
    assert!(drawn(cx, "session-menu"), "a's menu stays open");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));
    test_support::keys(cx, "escape");
    let shown = "Seen: seen-failure-sentinel";
    assert_eq!(label_of(cx, "session-outcome-b").as_deref(), Some(shown));
    test_support::click(cx, "session-menu-b");
    assert_eq!(
        label_of(cx, "session-menu-outcome-b").as_deref(),
        Some(shown)
    );
    test_support::keys(cx, "escape");
    push(
        &mut fake,
        &["a", "b"],
        &[(session("a").title("renamed"), 0)],
        cx,
    );
    assert_eq!(label_of(cx, "session-outcome-b").as_deref(), Some(shown));

    test_support::click(cx, "session-seen-b");
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-outcome-b"),
        None,
        "the refusal goes when the next toggle starts"
    );
    assert_eq!(answered(&mut fake), [seen_put("b", json!(250))]);
}

/// SPEC.md: a pending request refuses the session's other actions, and the brief for this port decided that a manual
/// toggle is such a request (the old app ran it beside them). Other sessions stay usable. Specified: with b's toggle
/// sent and unanswered, each of Rename, the toggle, Replace, Stop and Delete in b's menu ignores a click (the menu stays
/// open and asks nothing), and a click on b's dot sends nothing more and does not open b; a's Replace still asks; once
/// the toggle is answered, b's Replace asks.
#[gpui::test]
fn a_pending_toggle_makes_the_sessions_other_actions_unavailable(cx: &mut TestAppContext) {
    let rows = [(session("a"), 0), (unseen("b"), 0)];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    answered(&mut fake);
    test_support::click(cx, "session-seen-b");
    test_support::settle(cx);
    let [pending] = &fake.commands()[..] else {
        panic!("one toggle was sent");
    };
    let pending = pending.id;

    test_support::click(cx, "session-menu-b");
    for item in [
        "session-menu-rename",
        "session-menu-seen",
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
    test_support::click(cx, "session-seen-b");
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("a"), "the dot opened nothing");
    assert!(fake.commands().is_empty(), "nothing more was sent");

    test_support::click(cx, "session-menu-a");
    test_support::click(cx, "session-menu-replace");
    assert!(drawn(cx, "prompt"), "a's Replace still asks");
    test_support::click(cx, "prompt-cancel");

    fake.reply(pending, 200, None);
    test_support::settle(cx);
    test_support::click(cx, "session-menu-b");
    test_support::click(cx, "session-menu-replace");
    assert!(
        drawn(cx, "prompt"),
        "b's Replace asks once the toggle settled"
    );
}

// ---- The dot -------------------------------------------------------------------------------------------------------

/// SPEC.md: the toggle is "reachable there or by clicking the dot itself", and "A clickable dot also names its mark
/// read or mark unread action" after the status it explains. The old dot stopped its click so the row did not open,
/// and a click on a row is an open (a selection, a visit, a seen mark), so a dot that let the click through would open
/// the session and send writes the user did not ask for. Specified: with a open and its terminal focused, b's dot's
/// help is "Idle — new output — Mark read"; a click on it sends b's toggle once, and nothing else (no visit), leaves a
/// open and its terminal focused; once the helm pushes b seen, its help is "Idle — Mark unread". Every live row has a
/// dot, so its accessible name says whose it is: b's is "Mark read: b", then "Mark unread: b".
#[gpui::test]
fn the_dot_toggles_once_without_opening_its_row(cx: &mut TestAppContext) {
    let rows = [(session("a"), 0), (unseen("b"), 0)];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    answered(&mut fake);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("terminal-grid"),
        "the premise: a's terminal has focus"
    );
    assert_eq!(
        label_of(cx, "session-seen-b").as_deref(),
        Some("Mark read: b")
    );
    assert_eq!(
        help_after_crossing(cx, "session-row-b", "session-seen-b").as_deref(),
        Some("Idle — new output — Mark read")
    );

    test_support::click(cx, "session-seen-b");
    test_support::settle(cx);
    assert_eq!(answered(&mut fake), [seen_put("b", json!(250))]);
    assert_eq!(selected(cx).as_deref(), Some("a"));
    assert_eq!(focused_id(cx).as_deref(), Some("terminal-grid"));

    push(
        &mut fake,
        &["a", "b"],
        &[(unseen("b").seen(Some(250)), 0)],
        cx,
    );
    assert_eq!(
        label_of(cx, "session-seen-b").as_deref(),
        Some("Mark unread: b")
    );
    assert_eq!(
        help_after_crossing(cx, "session-row-b", "session-seen-b").as_deref(),
        Some("Idle — Mark unread")
    );
}

// ---- The open session's seen memory --------------------------------------------------------------------------------

/// SPEC.md, "Status": "A manual mark unread on the session currently open STICKS — the automatic rule does not
/// immediately re-record it as seen — until the user leaves and returns to it, or new output arrives." The automatic
/// rule runs on every pushed change, so each of those is a chance to undo the user's choice. Specified, with a open and
/// seen: Mark unread from a's menu sends `null`, and the helm's push of a unseen sends nothing; nor does a pushed change
/// to b; opening b sends only its visit, and opening a again sends its visit and its seen mark at its activity; a
/// second Mark unread, from a's dot, sticks the same way, and new output on a sends its seen mark at the new activity.
#[gpui::test]
fn a_manual_unread_of_the_open_session_sticks_until_the_user_returns_or_output_arrives(
    cx: &mut TestAppContext,
) {
    let rows = [(idle("a"), 0), (idle("b"), 0)];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    answered(&mut fake);
    let order = ["a", "b"];

    toggle_from_menu(cx, "session-menu-a");
    assert_eq!(answered(&mut fake), [seen_put("a", Value::Null)]);
    push(&mut fake, &order, &[(idle("a").seen(None), 0)], cx);
    assert_eq!(answered(&mut fake), [], "the helm's push undoes nothing");
    push(&mut fake, &order, &[(idle("b").title("renamed"), 0)], cx);
    assert_eq!(answered(&mut fake), [], "nor does a change elsewhere");
    assert_eq!(
        label_of(cx, "session-seen-a").as_deref(),
        Some("Mark read: a"),
        "a still reads unseen"
    );

    test_support::click(cx, "session-row-b");
    assert_eq!(answered(&mut fake), [remembered("b")]);
    test_support::click(cx, "session-row-a");
    assert_eq!(
        answered(&mut fake),
        [remembered("a"), seen_put("a", json!(100))],
        "returning to a marks it seen"
    );
    push(&mut fake, &order, &[(idle("a"), 0)], cx);

    test_support::click(cx, "session-seen-a");
    test_support::settle(cx);
    assert_eq!(answered(&mut fake), [seen_put("a", Value::Null)]);
    push(&mut fake, &order, &[(idle("a").seen(None), 0)], cx);
    assert_eq!(answered(&mut fake), []);
    push(
        &mut fake,
        &order,
        &[(idle("a").activity(150).seen(None), 0)],
        cx,
    );
    assert_eq!(
        answered(&mut fake),
        [seen_put("a", json!(150))],
        "new output is marked seen"
    );
}

/// The M1 follow-up: a listing that hides the open session (the helm's cap cut it) must not make the seen memory
/// forget it, or its return to the list reads as a new opening and marks it seen, undoing a manual Mark unread of a
/// session the user never left. The memory must still notice output that arrived while it was hidden. Specified, with
/// a open and marked unread by hand: a cut listing without a keeps a open and sends nothing; a complete listing with
/// a back, unseen at the same activity, sends nothing; after another cut, a's return with new activity sends its seen
/// mark at that activity.
#[gpui::test]
fn the_seen_memory_survives_a_cut_listing_that_hides_the_open_session(cx: &mut TestAppContext) {
    let rows = [(idle("a"), 0), (idle("b"), 0)];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    answered(&mut fake);
    toggle_from_menu(cx, "session-menu-a");
    push(&mut fake, &["a", "b"], &[(idle("a").seen(None), 0)], cx);
    assert_eq!(answered(&mut fake), [seen_put("a", Value::Null)]);

    let cut = || change(listing_meta(&["b"], 500, true), &[], &["a"]);
    fake.push(cut());
    test_support::settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "a cut listing proves nothing"
    );
    assert!(
        !drawn(cx, "session-row-a"),
        "the premise: the listing hides a"
    );
    assert_eq!(answered(&mut fake), []);

    fake.push(change(
        listing_meta(&["a", "b"], 2, false),
        &[idle("a").seen(None)],
        &[],
    ));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-seen-a").as_deref(),
        Some("Mark read: a"),
        "the premise: a is back, unseen"
    );
    assert_eq!(answered(&mut fake), [], "its return marks nothing");

    fake.push(cut());
    test_support::settle(cx);
    fake.push(change(
        listing_meta(&["a", "b"], 2, false),
        &[idle("a").activity(150).seen(None)],
        &[],
    ));
    test_support::settle(cx);
    assert_eq!(answered(&mut fake), [seen_put("a", json!(150))]);
}

/// The same follow-up through a filter: SPEC.md, "Hiding the selected session changes neither that selection nor its
/// open terminal", so choosing a host the open session is not on hides it without the user leaving it, and choosing
/// All brings it back. Specified, with a open and marked unread by hand, over the local host and one remote: choosing
/// the remote sends its session query; the helm's narrowed listing without a keeps a open and sends nothing; choosing
/// All and the helm's listing with a back, unseen at the same activity, send nothing but All's query.
#[gpui::test]
fn the_seen_memory_survives_a_host_filter_that_hides_the_open_session(cx: &mut TestAppContext) {
    let rows = [(idle("a"), 0), (idle("b"), 0)];
    let mut state = snapshot(&rows, false);
    state.push(hosts(&[
        local(),
        remote(2, "ops@render-west.example.invalid"),
    ]));
    let (mut fake, cx) = open_main(cx, state);
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    answered(&mut fake);
    toggle_from_menu(cx, "session-menu-a");
    push(&mut fake, &["a", "b"], &[(idle("a").seen(None), 0)], cx);
    assert_eq!(answered(&mut fake), [seen_put("a", Value::Null)]);

    test_support::click(cx, "host-rail-host-2");
    assert_eq!(
        answered(&mut fake),
        [(
            Method::Put,
            SESSION_QUERY_PATH.to_owned(),
            json!({ "host": 2 })
        )]
    );
    fake.push(change(narrowed(&["b"], 2, 1), &[], &["a"]));
    test_support::settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "a filter proves nothing"
    );
    assert!(
        !drawn(cx, "session-row-a"),
        "the premise: the listing hides a"
    );
    assert_eq!(answered(&mut fake), []);

    test_support::click(cx, "host-rail-all");
    fake.push(change(
        listing_meta(&["a", "b"], 2, false),
        &[idle("a").seen(None)],
        &[],
    ));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-seen-a").as_deref(),
        Some("Mark read: a"),
        "the premise: a is back, unseen"
    );
    assert_eq!(
        answered(&mut fake),
        [(Method::Put, SESSION_QUERY_PATH.to_owned(), json!({}))],
        "a's return marks nothing"
    );
}

/// SPEC.md, "Status": a manual Mark unread sticks "until the user leaves and returns to it", and a filter that hides
/// the open session must not hide the user leaving and coming back either. The quick switcher searches every host
/// whatever the sidebar shows, so with a host chosen the user can open sessions the list does not show. Found by the
/// review gate, as a memory kept for whichever hidden session was open. Specified, with a open and marked unread by
/// hand, over the local host and one remote: choosing the remote hides a and c; the quick switcher opens c, then a again,
/// each picked by typing its title and Enter, and nothing is marked while they are hidden; choosing All, with the
/// helm's listing showing a again unseen at the same activity, marks a seen at that activity.
#[gpui::test]
fn leaving_and_returning_while_the_session_is_hidden_marks_it_seen(cx: &mut TestAppContext) {
    let rows = [(idle("a"), 0), (idle("b"), 0), (idle("c"), 0)];
    let mut state = snapshot(&rows, false);
    state.push(hosts(&[
        local(),
        remote(2, "ops@render-west.example.invalid"),
    ]));
    let (mut fake, cx) = open_main(cx, state);
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    answered(&mut fake);
    toggle_from_menu(cx, "session-menu-a");
    let order = ["a", "b", "c"];
    push(&mut fake, &order, &[(idle("a").seen(None), 0)], cx);
    assert_eq!(answered(&mut fake), [seen_put("a", Value::Null)]);

    test_support::click(cx, "host-rail-host-2");
    answered(&mut fake);
    fake.push(change(narrowed(&["b"], 3, 1), &[], &["a", "c"]));
    test_support::settle(cx);
    assert!(
        !drawn(cx, "session-row-a") && !drawn(cx, "session-row-c"),
        "the premise: the listing hides a and c"
    );

    let fleet = [idle("a").seen(None), idle("b"), idle("c")];
    quick_switch(&mut fake, cx, "c", &fleet);
    assert_eq!(selected(cx).as_deref(), Some("c"), "the premise: c is open");
    quick_switch(&mut fake, cx, "a", &fleet);
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "the premise: a is open again"
    );
    assert!(
        !answered(&mut fake)
            .iter()
            .any(|(_, path, _)| path.ends_with("/seen")),
        "nothing is marked while hidden"
    );

    test_support::click(cx, "host-rail-all");
    fake.push(change(
        listing_meta(&order, 3, false),
        &[idle("a").seen(None), idle("c")],
        &[],
    ));
    test_support::settle(cx);
    assert_eq!(
        answered(&mut fake),
        [
            (Method::Put, SESSION_QUERY_PATH.to_owned(), json!({})),
            seen_put("a", json!(100)),
        ],
        "a's return after the user left it is marked seen"
    );
}

/// Open the quick switcher with its chord, answer its read with `fleet` (every command before the read answered as
/// done), type `title` and press Enter, which picks the first match: the session titled `title`, for these fixtures.
fn quick_switch(fake: &mut FakeHelm, cx: &mut VisualTestContext, title: &str, fleet: &[Fixture]) {
    answered(fake);
    test_support::keys(cx, QUICK_SWITCHER_CHORD);
    let [read] = &fake.commands()[..] else {
        panic!("the switcher sent one read");
    };
    assert_eq!(
        read.method,
        Method::Get,
        "the premise: {read:?} is the read"
    );
    let sessions: Vec<Value> = fleet.iter().map(|row| row.entry().value).collect();
    let total = sessions.len();
    fake.reply(
        read.id,
        200,
        Some(json!({ "sessions": sessions, "total": total, "truncated": false })),
    );
    test_support::settle(cx);
    test_support::keys(cx, title);
    test_support::keys(cx, "enter");
    test_support::settle(cx);
}

/// The sessions collection's metadata for a host query that matched `matching` of `total` sessions, complete.
fn narrowed(order: &[&str], total: u64, matching: u64) -> CollectionMeta {
    let mut meta = meta(order);
    meta.counts = Some(ListCounts {
        total,
        matching: Some(matching),
        truncated: false,
    });
    meta
}

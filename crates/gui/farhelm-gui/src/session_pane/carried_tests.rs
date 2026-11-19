//! Tests of the open session while the list does not show it: a status filter leaves it out of the listing, and the
//! helm carries it in the pushed collection outside the list's order because the session query names it
//! (`session_list::filters`, "The query names the open session"). The list must not draw or count it, and the pane must
//! work on it as on a listed session: its terminal stays, the header's actions ask and send about it, the details bar
//! names it, and the Rename shortcut renames it. Once it is deleted, by this window or elsewhere, the selection moves
//! on, filtered listing or not; an absence the helm says its host's cap cut away keeps it open.
//!
//! The window is the real main view over the helm client's fake, which plays the helm's side: the test pushes what the
//! helm would push for each query and answers every command itself. The fixtures are the rail's: sessions a and b on
//! host 2, a open (the last visited), over the rail's fleet of hosts. Every step ends in one of the shared harness's
//! step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::CollectionMeta;
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::details::TITLE_ID;
use super::header::Action;
use crate::actions::RENAME_SESSION_CHORD;
use crate::session_list::host_rail::tests::{
    answer_everything, filtered, fleet_hosts, hosts, label, node, one_query, sent_queries,
};
use crate::session_list::menu_tests::drawn;
use crate::session_list::tests::{
    Fixture, carrying, change, listing, listing_meta, open_main, painted, preferences, selected,
    session,
};
use crate::test_support::{self, any_open, frame, keys, settle};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The window over `a` (open, the last visited) and `b`, with every command of the startup answered.
fn open_a(cx: &mut TestAppContext, a: Fixture, b: Fixture) -> (FakeHelm, &mut VisualTestContext) {
    let (mut fake, cx) = open_main(
        cx,
        vec![
            listing(&["a", "b"], &[a, b]),
            preferences(&["a"], None),
            hosts(&fleet_hosts()),
        ],
    );
    answer_everything(&mut fake);
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    (fake, cx)
}

/// Choose Waiting, which matches neither session, and answer as the helm does: the query names a, so the listing
/// (0 of 2 matched, complete) leaves both out and carries a outside its order.
fn hide_a(fake: &mut FakeHelm, cx: &mut VisualTestContext) {
    test_support::click(cx, "session-status-Waiting");
    let (query, body) = one_query(fake);
    assert_eq!(body, json!({ "status": "waiting", "selected": "a" }));
    fake.push(change(carrying(filtered(&[], 2, 0), "a"), &[], &["b"]));
    fake.reply(query.id, 204, None);
    settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "the premise: a stays open"
    );
}

/// The commands the window sent since the last look, as (method, path, body), left unanswered.
fn sent(fake: &mut FakeHelm) -> Vec<(Method, String, serde_json::Value)> {
    fake.commands()
        .into_iter()
        .map(|command| {
            (
                command.method,
                command.path,
                command.body.unwrap_or_default(),
            )
        })
        .collect()
}

/// With a idle and open, choose Running and answer as the helm does: the query names a, so the listing (1 of 2 matched,
/// complete) lists b alone and carries a outside its order.
fn hide_idle_a(fake: &mut FakeHelm, cx: &mut VisualTestContext) {
    test_support::click(cx, "session-status-Running");
    let (query, body) = one_query(fake);
    assert_eq!(body, json!({ "status": "running", "selected": "a" }));
    fake.push(change(carrying(filtered(&["b"], 2, 1), "a"), &[], &[]));
    fake.reply(query.id, 204, None);
    settle(cx);
    assert!(
        node(&frame(cx), "session-row-a").is_none(),
        "the premise: Running hides a"
    );
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "the premise: a stays open"
    );
}

/// Ask the header's Delete of the open a and confirm it; the Delete it sent, left unanswered.
fn confirm_delete_of_a(fake: &mut FakeHelm, cx: &mut VisualTestContext) -> FakeCommand {
    test_support::click(cx, Action::Delete.id());
    assert!(drawn(cx, "prompt"), "the header's Delete asks");
    test_support::click(cx, "prompt-confirm");
    let mut commands = fake.commands();
    assert_eq!(commands.len(), 1, "one Delete was sent: {commands:?}");
    let delete = commands.remove(0);
    assert_eq!(
        (delete.method, delete.path.split('?').next()),
        (Method::Delete, Some("/api/sessions/a"))
    );
    delete
}

// ---- The open session the list leaves out --------------------------------------------------------------------------

/// SPEC.md: "Hiding the selected session changes neither that selection nor its open terminal" (the old status spec's
/// filtered-out selected session, whose terminal stayed and whose pane still named it), and Rename's shortcut "opens
/// Rename for the open session, including when its row is filtered out" (the old `session-action-shortcuts.spec.ts`
/// case "browser Rename chord edits the filtered open session"). The header and the details bar read the same row, so
/// before the helm carried it they drew the actions disabled and no bar, and Rename did nothing. Specified, with
/// Waiting hiding a ("Hidden work") and the helm carrying it: a stays selected and nothing but the query was sent (no
/// selection is written or cleared); the list draws no row for a, counts "0 of 2" and says "No waiting sessions."; a's
/// terminal is still drawn and the details bar names "Hidden work"; the Rename shortcut opens the editor, and a typed
/// title sends `{"title": "renamed"}` to a's rename route; the helm's push of the renamed carried a and its reply close
/// the editor, the details bar names "renamed", and the list still draws no row for a; the header's Stop then asks (a
/// lists a tab), and Confirm sends a's Stop.
#[gpui::test]
fn a_filtered_out_open_session_keeps_its_terminal_header_details_bar_and_rename(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_a(cx, session("a").title("Hidden work").tabs(1), session("b"));
    hide_a(&mut fake, cx);
    assert_eq!(sent(&mut fake), [], "nothing but the query was sent");
    let nodes = frame(cx);
    assert!(node(&nodes, "session-row-a").is_none(), "no row for a");
    assert!(
        painted(&nodes).iter().any(|text| text == "0 of 2"),
        "the count is the query's own: {:?}",
        painted(&nodes)
    );
    assert_eq!(label(&nodes, "filter-empty"), "No waiting sessions.");
    assert!(
        node(&nodes, "terminal-grid").is_some(),
        "a's terminal is still drawn"
    );
    assert_eq!(
        test_support::label(cx, TITLE_ID).as_deref(),
        Some("Hidden work")
    );

    keys(cx, RENAME_SESSION_CHORD);
    assert!(any_open(cx), "the shortcut opened Rename for a");
    crate::test_support::select_all_text(cx);
    keys(cx, "r e n a m e d enter");
    let commands = fake.commands();
    let [rename] = &commands[..] else {
        panic!("one rename was sent: {commands:?}");
    };
    assert_eq!(
        (
            rename.method,
            rename.path.as_str(),
            rename.body.clone().unwrap_or_default()
        ),
        (
            Method::Post,
            "/api/sessions/a/rename",
            json!({ "title": "renamed" })
        )
    );
    fake.push(change(
        carrying(filtered(&[], 2, 0), "a"),
        &[session("a").title("renamed").tabs(1)],
        &[],
    ));
    fake.reply(rename.id, 200, None);
    settle(cx);
    assert!(!any_open(cx), "the save closed the editor");
    assert_eq!(
        test_support::label(cx, TITLE_ID).as_deref(),
        Some("renamed")
    );
    assert!(node(&frame(cx), "session-row-a").is_none(), "still no row");

    test_support::click(cx, Action::Stop.id());
    assert!(drawn(cx, "prompt"), "the premise: Stop asks about a's tabs");
    test_support::click(cx, "prompt-confirm");
    let stops: Vec<_> = sent(&mut fake)
        .into_iter()
        .map(|(method, path, _)| (method, path))
        .collect();
    assert_eq!(stops, [(Method::Post, "/api/sessions/a/stop".to_owned())]);
}

/// The gate's finding on the first version of this work: once the header could delete a hidden open session, the helm
/// stopped carrying it, no filtered listing proves a session gone, and the pane stayed on the deleted session for good
/// ("Session unavailable — reconnecting", an empty header, no bar) until the user chose All. The helm's push without
/// the session its answered query names is the proof (`session_list::selection`, "A selection stays until it is proven
/// gone"), and the automatic choice then waits for the Delete as after any Delete. Specified, with a idle and b
/// running, Running hiding a and listing b (1 of 2) while the helm carries a: the header's Delete asks, Confirm sends
/// a's Delete; the helm's push removing a clears the selection while the Delete is pending, and its success reply then
/// opens b, which the details bar names and whose header draws its actions.
#[gpui::test]
fn this_windows_delete_of_a_hidden_open_session_opens_the_next_session(cx: &mut TestAppContext) {
    let idle = json!({ "state": "idle" });
    let (mut fake, cx) = open_a(cx, session("a").status(idle), session("b"));
    hide_idle_a(&mut fake, cx);
    let delete = confirm_delete_of_a(&mut fake, cx);
    // The helm pushes the removal before its reply; settling in between shows that the pending Delete holds the
    // automatic choice back (SPEC.md: "Automatic selection after Delete waits until every admitted Delete has
    // settled"), so a is dropped and b is not opened yet.
    fake.push(change(filtered(&["b"], 1, 1), &[], &["a"]));
    settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        None,
        "a pending Delete holds the next session back"
    );
    fake.reply(delete.id, 200, Some(json!({})));
    settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some("b"),
        "the next session opened"
    );
    assert_eq!(test_support::label(cx, TITLE_ID).as_deref(), Some("b"));
    assert!(
        cx.debug_bounds(Action::Delete.id()).is_some(),
        "b's header draws its actions"
    );
}

/// The helm's push without the open session proves it gone only while no later query is on its way, so a Delete the
/// user follows with another filter choice would leave the pane on the deleted session, as unavailable, until that
/// query's reply, one round trip later. This window's own Delete that the helm did is the proof then
/// (`session_list::delete`, "This window's own Delete is evidence"). Specified, with a idle and b running, Running
/// hiding a and listing b while the helm carries a: Confirm sends a's Delete; a click on All then sends
/// `{"selected": "a"}`; the helm, taking commands in order, pushes the removal of a under Running and replies to the
/// Delete, and b opens while the All query is still unanswered; once the helm answers it, b stays open.
#[gpui::test]
fn this_windows_delete_proves_its_session_gone_while_a_later_query_is_in_flight(
    cx: &mut TestAppContext,
) {
    let idle = json!({ "state": "idle" });
    let (mut fake, cx) = open_a(cx, session("a").status(idle), session("b"));
    hide_idle_a(&mut fake, cx);
    let delete = confirm_delete_of_a(&mut fake, cx);
    test_support::click(cx, "session-status-All");
    let (all, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "selected": "a" }));

    fake.push(change(filtered(&["b"], 1, 1), &[], &["a"]));
    fake.reply(delete.id, 200, Some(json!({})));
    settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some("b"),
        "the next session opened before the All query's reply"
    );

    fake.push(change(listing_meta(&["b"], 1, false), &[], &[]));
    fake.reply(all.id, 204, None);
    settle(cx);
    assert_eq!(sent_queries(&mut fake), [json!({ "selected": "b" })]);
    settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("b"));
}

/// The helm pushes a query's listing before it replies, so a query naming another session can leave the open session
/// out although it exists. A user who opens b and comes straight back to a, before the helm has answered the query
/// naming b, must not lose a to that listing: the proof waits until nothing is in flight
/// (`session_list::filters::Choice::serves_named`'s NOTE). Specified, with a and b running and Running answered naming
/// a (both listed): clicks on b and then a; the helm's listing for the query naming b, in which a (gone idle) is
/// neither listed nor carried, and its reply keep a open; the helm's listing for the query naming a, carrying the idle
/// a, and its reply keep a open with the details bar naming it.
#[gpui::test]
fn a_quick_return_to_a_session_is_not_undone_by_the_query_it_overtook(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_a(cx, session("a").title("Return"), session("b"));
    test_support::click(cx, "session-status-Running");
    let (query, _) = one_query(&mut fake);
    fake.push(change(filtered(&["a", "b"], 2, 2), &[], &[]));
    fake.reply(query.id, 204, None);
    settle(cx);

    test_support::click(cx, "session-row-b");
    test_support::click(cx, "session-row-a");
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "the premise: a is open again"
    );
    let commands = fake.commands();
    let naming = |key: &str| {
        commands
            .iter()
            .position(|command| {
                command.body == Some(json!({ "status": "running", "selected": key }))
            })
            .unwrap_or_else(|| panic!("a query naming {key}: {commands:?}"))
    };
    let (for_b, for_a) = (naming("b"), naming("a"));
    assert!(for_b < for_a, "the premise: the query naming b went first");

    fake.push(change(filtered(&["b"], 2, 1), &[], &["a"]));
    for command in &commands[..=for_b] {
        fake.reply(command.id, 204, None);
    }
    settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "the query naming a is still on its way"
    );

    let idle = json!({ "state": "idle" });
    fake.push(change(
        carrying(filtered(&["b"], 2, 1), "a"),
        &[session("a").title("Return").status(idle)],
        &[],
    ));
    for command in &commands[for_b + 1..] {
        fake.reply(command.id, 204, None);
    }
    settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("a"));
    assert_eq!(test_support::label(cx, TITLE_ID).as_deref(), Some("Return"));
}

/// SPEC.md's exception to "Existing removal detection still needs a complete, unfiltered fleet reply" (decided by the
/// maintainer on 2026-11-15): once the helm has answered a query naming the open session, it carries that session in
/// every listing while it exists, so a filtered listing without it means someone else deleted it (an agent, the command
/// line). Before, the pane stayed on the deleted session as an unavailable pane, with no header actions and no details
/// bar, until the user chose All. Specified, with a idle and b running, Running hiding a and listing b (1 of 2) while
/// the helm carries a, and nothing deleted from this window: a push that removes a and carries nothing opens b, which
/// the details bar names and whose header draws its actions, and the query is sent again as Running naming b.
#[gpui::test]
fn a_carried_session_deleted_elsewhere_moves_the_selection_on(cx: &mut TestAppContext) {
    let idle = json!({ "state": "idle" });
    let (mut fake, cx) = open_a(cx, session("a").status(idle), session("b"));
    hide_idle_a(&mut fake, cx);

    fake.push(change(filtered(&["b"], 1, 1), &[], &["a"]));
    settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some("b"),
        "the next session opened"
    );
    assert_eq!(test_support::label(cx, TITLE_ID).as_deref(), Some("b"));
    assert!(
        cx.debug_bounds(Action::Delete.id()).is_some(),
        "b's header draws its actions"
    );
    assert_eq!(
        sent_queries(&mut fake),
        [json!({ "status": "running", "selected": "b" })]
    );
}

/// The contrast to `a_carried_session_deleted_elsewhere_moves_the_selection_on`: each host lists only its newest 500
/// sessions, so an old open session on a host past its cap is in neither a filtered listing nor the helm's read of it
/// alone, although it exists. With a host filter naming another host, the listing was read whole and its counts say
/// nothing about the open session's host, so the helm marks the absence itself (`CollectionMeta::selected_cut`), and
/// the window must take that as no proof (`session_list::selection::absent_while_named`'s NOTE). Before the mark, the
/// window moved on from a session that still existed. Specified, with a on host 2 and b on host 3, and host 3 chosen
/// on the rail (`{"host": 3, "selected": "a"}`) and the helm carrying a (the details bar names it): a push that lists b
/// alone, carries nothing, removes a from the store (the details bar draws nothing) and says the open session's read
/// was cut keeps a open, and the window sends nothing, neither the query again nor a new selection.
#[gpui::test]
fn a_carried_session_cut_away_on_its_own_host_stays_open(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_a(cx, session("a").host(2), session("b").host(3));
    test_support::click(cx, "host-rail-host-3");
    let (query, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "host": 3, "selected": "a" }));
    fake.push(change(carrying(filtered(&["b"], 2, 1), "a"), &[], &[]));
    fake.reply(query.id, 204, None);
    settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "the premise: a is open while host 3 is chosen"
    );
    assert_eq!(
        test_support::label(cx, TITLE_ID).as_deref(),
        Some("a"),
        "the premise: the helm carries a, so the details bar names it"
    );

    let cut = CollectionMeta {
        selected_cut: true,
        ..filtered(&["b"], 2, 1)
    };
    fake.push(change(cut, &[], &["a"]));
    settle(cx);
    // The details bar draws nothing for a session the store holds no row for, so this shows the push landed.
    assert_eq!(
        test_support::label(cx, TITLE_ID),
        None,
        "the push removed a's row"
    );
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "an absence cut away is no proof"
    );
    assert_eq!(sent(&mut fake), [], "nothing was sent");
}

/// The helm carries the open session's row outside the list while a filter hides it, and keeps marking it stale with
/// its host like any row, so the pane must show the unavailable card for it as for a listed session: the card reads the
/// row by key, never the listing. Specified: with a open and live, and a status filter hiding it (carried), a push that
/// carries a stale draws the unavailable card over a's veiled terminal.
#[gpui::test]
fn a_carried_open_session_that_goes_stale_shows_the_unavailable_card(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_a(cx, session("a"), session("b"));
    hide_a(&mut fake, cx);
    assert!(
        !drawn(cx, super::unavailable::CARD_ID),
        "the premise: no card yet"
    );
    fake.push(change(
        carrying(filtered(&[], 2, 0), "a"),
        &[session("a").stale()],
        &[],
    ));
    settle(cx);
    assert!(drawn(cx, super::unavailable::CARD_ID));
    assert!(drawn(cx, crate::terminal::attached::INERT_SCRIM_ID));
}

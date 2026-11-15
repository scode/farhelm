//! Tests of the open session while the list does not show it: a status filter leaves it out of the listing, and the
//! helm carries it in the pushed collection outside the list's order because the session query names it
//! (`session_list::filters`, "The query names the open session"). The list must not draw or count it, and the pane must
//! work on it as on a listed session: its terminal stays, the header's actions ask and send about it, the details bar
//! names it, and the Rename shortcut renames it. This window's own Delete of it moves the selection on; a deletion made
//! elsewhere does not, until a complete listing proves it.
//!
//! The window is the real main view over the helm client's fake, which plays the helm's side: the test pushes what the
//! helm would push for each query and answers every command itself. The fixtures are the rail's: sessions a and b on
//! host 2, a open (the last visited), over the rail's fleet of hosts. Every step ends in one of the shared harness's
//! step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::details::{BAR_ID, TITLE_ID};
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

/// Kit's select-all in a text field on this platform.
const SELECT_ALL: &str = if cfg!(target_os = "macos") {
    "cmd-a"
} else {
    "ctrl-a"
};

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
    keys(cx, SELECT_ALL);
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
/// ("Session unavailable — reconnecting", an empty header, no bar) until the user chose All. This window's own Delete
/// that the helm did is the proof (`session_list::delete`, "This window's own Delete is evidence"). Specified, with a
/// idle and b running, Running hiding a and listing b (1 of 2) while the helm carries a: the header's Delete asks,
/// Confirm sends a's Delete; the helm's push removing a leaves a selected while the Delete is pending, and its success
/// reply then opens b, which the details bar names and whose header draws its actions.
#[gpui::test]
fn this_windows_delete_of_a_hidden_open_session_opens_the_next_session(cx: &mut TestAppContext) {
    let idle = json!({ "state": "idle" });
    let (mut fake, cx) = open_a(cx, session("a").status(idle), session("b"));
    test_support::click(cx, "session-status-Running");
    let (query, _) = one_query(&mut fake);
    fake.push(change(carrying(filtered(&["b"], 2, 1), "a"), &[], &[]));
    fake.reply(query.id, 204, None);
    settle(cx);
    assert!(
        node(&frame(cx), "session-row-a").is_none(),
        "the premise: Running hides a"
    );

    test_support::click(cx, Action::Delete.id());
    assert!(drawn(cx, "prompt"), "the header's Delete asks");
    test_support::click(cx, "prompt-confirm");
    let commands = fake.commands();
    let [delete] = &commands[..] else {
        panic!("one Delete was sent: {commands:?}");
    };
    assert_eq!(
        (delete.method, delete.path.split('?').next()),
        (Method::Delete, Some("/api/sessions/a"))
    );
    // The helm pushes the removal before its reply; settling in between shows that only the Delete's success moves
    // the selection, never the store losing the session while the Delete is pending.
    fake.push(change(filtered(&["b"], 1, 1), &[], &["a"]));
    settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "a pending Delete moves nothing"
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

/// SPEC.md: "Existing removal detection still needs a complete, unfiltered fleet reply to infer an external deletion",
/// so a carried session that disappears because someone else deleted it (another window, the command line) is not
/// proof, and the open session stays open as an unavailable pane until a complete listing shows it gone. Letting the
/// carried session's absence count is with the maintainer (raised by the migration's orchestrator on 2026-11-14), and
/// the parity file records the gap. Meanwhile nothing reads a row that is not there. Specified, with Waiting hiding a
/// and the helm carrying it: a push that removes a and carries nothing keeps a selected, with no details bar, no header
/// actions and nothing opened by the Rename shortcut; choosing All sends `{"selected": "a"}`, and its complete listing
/// of b alone opens b, for which the query is sent again as `{"selected": "b"}`.
#[gpui::test]
fn a_carried_session_deleted_elsewhere_stays_open_until_a_complete_listing_proves_it_gone(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_a(cx, session("a").title("Hidden work"), session("b"));
    hide_a(&mut fake, cx);
    fake.push(change(filtered(&[], 1, 0), &[], &["a"]));
    settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("a"));
    assert!(node(&frame(cx), BAR_ID).is_none(), "no row, no bar");
    for action in Action::ALL {
        assert!(
            cx.debug_bounds(action.id()).is_none(),
            "{action:?} is not drawn"
        );
    }
    keys(cx, RENAME_SESSION_CHORD);
    assert!(!any_open(cx), "nothing to rename");
    assert_eq!(sent(&mut fake), []);

    test_support::click(cx, "session-status-All");
    let (query, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "selected": "a" }));
    fake.push(change(listing_meta(&["b"], 1, false), &[session("b")], &[]));
    fake.reply(query.id, 204, None);
    settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("b"));
    assert_eq!(sent_queries(&mut fake), [json!({ "selected": "b" })]);
}

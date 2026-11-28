//! Tests of a blocked session in the session list: a session whose agent ran `farhelm agent notify`, which the helm
//! reports as `SessionRow::blocked` (SPEC.md, "Status").
//!
//! The drawing rules are pure functions of one row ([`status::live`], [`mute::after_title`]), so they are pinned on
//! rows decoded from the same fixtures the window tests push ([`Fixture::row`]). What the window sends when the user
//! switches to a blocked session (a row's click, a quick switcher pick) runs in the real main view over the helm
//! client's fake, reading the commands it sent; the window's activity is pinned through the ticker's override, never
//! read from the test platform. The local filter's answer is pinned in `filters`' own tests and the collapsed group's
//! mark in `groups_tests`.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::{Lane, Method};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::host_rail::tests::filtered;
use super::menu_tests::snapshot;
use super::status::{self, Live};
use super::tests::{Fixture, acknowledge_commands, change, open_main, selected, session};
use super::{mute, selection};
use crate::{test_support, ticker};

/// A session in the status the helm's word `state` names.
fn in_state(key: &'static str, state: &str) -> Fixture {
    session(key).status(json!({ "state": state }))
}

/// The attention commands among those sent since the last look, as (path, body, lane), each answered with success
/// like everything else sent, so nothing waits behind them.
fn attention_sent(fake: &mut FakeHelm) -> Vec<(String, Value, Lane)> {
    acknowledge_commands(fake)
        .into_iter()
        .filter(|command| command.path.ends_with("/attention"))
        .map(|command| {
            assert_eq!(command.method, Method::Post, "{}", command.path);
            (command.path, command.body.unwrap_or_default(), command.lane)
        })
        .collect()
}

/// The main view over `rows`, the first open, with the window counted as `active`.
fn open<'a>(
    cx: &'a mut TestAppContext,
    rows: &[Fixture],
    active: bool,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let rows: Vec<(Fixture, usize)> = rows.iter().map(|row| (row.clone(), 0)).collect();
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    cx.update(|_, cx| ticker::set_active_override(Some(active), cx));
    test_support::settle(cx);
    acknowledge_commands(&mut fake);
    (fake, cx)
}

/// The acceptance criteria ask for the exact existing waiting mark for a blocked session, whatever its supervisor
/// reports, and never for an agent that has ended, so the list cannot show an exited session as asking anything.
/// Specified: a blocked running, idle (with unseen output) or unclassified row draws the waiting mark, which does not
/// pulse; a blocked row that exited draws no live mark and keeps its ended wording; an unblocked running row still
/// draws the running mark.
#[test]
fn a_blocked_live_or_unclassified_row_draws_the_waiting_mark() {
    for fixture in [
        in_state("running", "running").blocked(),
        in_state("idle", "idle").activity(250).blocked(),
        in_state("unknown", "unknown").blocked(),
    ] {
        let row = fixture.row();
        assert_eq!(status::live(&row), Some(Live::Waiting), "{}", fixture.key);
        assert!(!status::pulses(&row), "{}", fixture.key);
    }
    let exited = session("exited")
        .status(json!({"state": "exited", "exit_code": 0}))
        .blocked()
        .row();
    assert_eq!(status::live(&exited), None);
    assert!(status::ended_words(&exited, false).is_some());
    assert_eq!(
        status::live(&in_state("plain", "running").row()),
        Some(Live::Running)
    );
}

/// A muted session that is waiting on a connected host keeps its red dot with a bell after its title, because a
/// question breaks through a mute; a blocked session is the same kind of request, so it must break through too, and a
/// stale one, last-known, must not. Specified: a muted, blocked, running row on a connected host has the bell after its
/// title and not in the dot's place; the same row on an unreachable host has the bell in the dot's place instead.
#[test]
fn a_muted_blocked_row_keeps_its_red_dot_with_the_bell_after_its_title() {
    let connected = in_state("live", "running").muted(None).blocked().row();
    assert!(mute::after_title(&connected));
    assert!(!mute::replaces_dot(&connected));
    let stale = in_state("stale", "running")
        .host(3)
        .stale()
        .muted(None)
        .blocked()
        .row();
    assert!(!mute::after_title(&stale));
    assert!(mute::replaces_dot(&stale));
}

/// Switching to a blocked session is what tells the helm the user has seen it, which clears its mark; it must come
/// only from the user's own switch in an active window, and ordinary switching must cost no traffic. Specified: with
/// `a` open in an active window, clicking the blocked row `b` sends `POST /api/sessions/b/attention` with
/// `{"kind": "switch"}` on the unordered lane and opens `b`; clicking the unblocked row `c` sends none; clicking `b`
/// again, now open, sends it again.
#[gpui::test]
fn the_users_switch_to_a_blocked_session_tells_the_helm(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(
        cx,
        &[session("a"), session("b").blocked(), session("c")],
        true,
    );
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");

    test_support::click(cx, "session-row-b");
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("b"));
    assert_eq!(
        attention_sent(&mut fake),
        [(
            "/api/sessions/b/attention".to_owned(),
            json!({"kind": "switch"}),
            Lane::Unordered
        )]
    );

    test_support::click(cx, "session-row-c");
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("c"));
    assert_eq!(attention_sent(&mut fake), [], "c is not blocked");

    test_support::click(cx, "session-row-b");
    test_support::settle(cx);
    test_support::click(cx, "session-row-b");
    test_support::settle(cx);
    assert_eq!(
        attention_sent(&mut fake).len(),
        2,
        "each switch to the blocked session says so, the open one's included"
    );
}

/// A window that is not active is not evidence the user is looking, and an automatic choice is not the user's at all.
/// Specified: with the window inactive, clicking the blocked row `b` opens it and sends no attention; with the window
/// active, opening `b` through the selection rules' automatic path sends none either.
#[gpui::test]
fn no_attention_from_an_inactive_window_or_an_automatic_choice(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, &[session("a"), session("b").blocked()], false);
    test_support::click(cx, "session-row-b");
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b opened");
    assert_eq!(attention_sent(&mut fake), []);

    cx.update(|_, cx| ticker::set_active_override(Some(true), cx));
    test_support::click(cx, "session-row-a");
    test_support::settle(cx);
    acknowledge_commands(&mut fake);
    cx.update(|_, cx| selection::open_automatically("b", cx));
    test_support::settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some("b"),
        "the premise: b opened again"
    );
    assert_eq!(attention_sent(&mut fake), []);
}

/// Open the quick switcher with its chord, answer its read with `fleet` (every other command answered as done), type
/// `title` and press Enter, which picks the session titled `title`. The switcher reads every session on every host, so
/// `fleet` may hold sessions the sidebar does not.
fn pick_in_switcher(
    fake: &mut FakeHelm,
    cx: &mut VisualTestContext,
    title: &str,
    fleet: &[Fixture],
) {
    acknowledge_commands(fake);
    test_support::keys(cx, crate::actions::QUICK_SWITCHER_CHORD);
    // Anything else the window sends meanwhile (a pick's widened session query, say) is answered as done.
    let mut reads = Vec::new();
    for command in fake.commands() {
        if command.method == Method::Get {
            reads.push(command);
        } else {
            fake.reply(command.id, 200, None);
        }
    }
    let [read] = &reads[..] else {
        panic!("the switcher sent one read: {reads:?}");
    };
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

/// The quick switcher finds sessions the sidebar does not hold (another host, a status filter, the listing cap), and
/// draws a blocked one with its red mark from its own read, so picking it must tell the helm too, judged by the
/// switcher's row when the sidebar has none. Specified: with the sidebar narrowed to Running, which leaves out the
/// blocked session `b` (a blocked session is never Running), picking `b` in the switcher opens it and sends
/// `{"kind": "switch"}` for `b`; picking the unblocked, listed `c` the same way sends none.
#[gpui::test]
fn a_quick_switcher_pick_of_a_blocked_session_the_sidebar_lacks_tells_the_helm(
    cx: &mut TestAppContext,
) {
    let blocked = session("b").title("Blocked build").blocked();
    let quiet = session("c").title("Quiet build");
    let (mut fake, cx) = open(cx, &[session("a"), blocked.clone(), quiet.clone()], true);
    test_support::click(cx, "session-status-Running");
    test_support::settle(cx);
    acknowledge_commands(&mut fake);
    fake.push(change(filtered(&["a", "c"], 3, 2), &[], &["b"]));
    test_support::settle(cx);
    assert!(
        !test_support::drawn(cx, "session-row-b"),
        "the premise: the sidebar leaves b out"
    );

    let fleet = [session("a"), blocked, quiet];
    pick_in_switcher(&mut fake, cx, "Blocked build", &fleet);
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b opened");
    assert_eq!(
        attention_sent(&mut fake),
        [(
            "/api/sessions/b/attention".to_owned(),
            json!({"kind": "switch"}),
            Lane::Unordered
        )]
    );

    pick_in_switcher(&mut fake, cx, "Quiet build", &fleet);
    assert_eq!(selected(cx).as_deref(), Some("c"), "the premise: c opened");
    assert_eq!(attention_sent(&mut fake), [], "c is not blocked");
}

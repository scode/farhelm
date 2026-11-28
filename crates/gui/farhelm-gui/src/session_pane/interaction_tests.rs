//! Tests of the pane's interaction signal (the pane's module docs, "Interaction is attention"): what the window tells
//! the helm when the user acts inside the open session while it reads as waiting, which withdraws the session's ntfy
//! alert during its grace period.
//!
//! The window is the real main view over the helm client's fake, as in the session list's blocked-session tests; the
//! window's activity is pinned through the ticker's override, never read from the test platform, and the throttle's
//! time is the test executor's clock, moved by hand.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::{Lane, Method};
use gpui::{Modifiers, ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext, point, px};
use serde_json::json;

use super::INTERACTION_GAP;
use crate::session_list::menu_tests::snapshot;
use crate::session_list::tests::{Fixture, acknowledge_commands, open_main, selected, session};
use crate::{test_support, ticker};

/// The interaction signals sent since the last look, by the path they went to, after checking each is a `POST` of
/// `{"kind": "interaction"}` with no age (sent at once) on the unordered lane. Everything sent is answered with
/// success, so nothing waits behind it. Switch signals and trailing signals would fail the check; [`aged`] reads the
/// latter.
fn interactions(fake: &mut FakeHelm) -> Vec<String> {
    aged(fake)
        .into_iter()
        .map(|(path, age)| {
            assert_eq!(age, None, "{path} was sent at once");
            path
        })
        .collect()
}

/// [`interactions`], with each signal's reported age in milliseconds (`None`: sent at once).
fn aged(fake: &mut FakeHelm) -> Vec<(String, Option<u64>)> {
    acknowledge_commands(fake)
        .into_iter()
        .filter(|command| command.path.ends_with("/attention"))
        .map(|command| {
            assert_eq!(command.method, Method::Post, "{}", command.path);
            assert_eq!(command.lane, Lane::Unordered, "{}", command.path);
            let body = command.body.clone().unwrap_or_default();
            assert_eq!(body["kind"], json!("interaction"), "{}", command.path);
            (
                command.path,
                body.get("age_ms").and_then(|age| age.as_u64()),
            )
        })
        .collect()
}

/// The main view over `rows`, the first open, with the window counted as `active`, and what it sent on opening
/// answered.
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

/// One wheel step at the center of the element drawn under `selector`, then settle.
fn scroll_over(cx: &mut VisualTestContext, selector: &'static str) {
    let at = test_support::center(cx, selector);
    cx.simulate_event(ScrollEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(0.), px(-40.))),
        modifiers: Modifiers::none(),
        ..Default::default()
    });
    test_support::settle(cx);
}

/// The value of the signal is that it comes from the user acting on the waiting session itself, so a press, a wheel
/// step and a key inside the open session must each say so. Specified: with blocked `a` open in an active window,
/// nothing is sent while it is merely open; a click inside the pane sends `POST /api/sessions/a/attention` with
/// `{"kind": "interaction"}` on the unordered lane; after the throttle's gap a wheel step over the pane sends another,
/// and after another gap a key typed into its terminal another.
#[gpui::test]
fn acting_inside_a_waiting_open_session_tells_the_helm(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, &[session("a").blocked(), session("b")], true);
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    test_support::advance(cx, INTERACTION_GAP * 2);
    assert_eq!(
        interactions(&mut fake),
        Vec::<String>::new(),
        "being open is not evidence"
    );

    test_support::click(cx, "main-pane");
    assert_eq!(interactions(&mut fake), ["/api/sessions/a/attention"]);

    test_support::advance(cx, INTERACTION_GAP);
    scroll_over(cx, "main-pane");
    assert_eq!(interactions(&mut fake), ["/api/sessions/a/attention"]);

    test_support::advance(cx, INTERACTION_GAP);
    assert!(
        test_support::terminal_focused(cx),
        "the premise: the click left the terminal focused"
    );
    test_support::keys(cx, "x");
    test_support::settle(cx);
    assert_eq!(interactions(&mut fake), ["/api/sessions/a/attention"]);
}

/// The signal withdraws an alert, so it must not come from anything that is not the user looking at a waiting
/// session. Specified: a click and a wheel step inside an open session that does not read as waiting send nothing; a
/// wheel step over the session list, outside the pane, sends nothing while blocked `b` is open; and in a window that
/// is not active a click inside blocked `b`'s pane sends nothing.
#[gpui::test]
fn no_interaction_signal_outside_a_waiting_session_or_an_active_window(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, &[session("a"), session("b").blocked()], true);
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    test_support::click(cx, "main-pane");
    scroll_over(cx, "main-pane");
    assert_eq!(
        interactions(&mut fake),
        Vec::<String>::new(),
        "a is not waiting"
    );

    test_support::click(cx, "session-row-b");
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    // The switch's own signal, which `interactions` would refuse.
    acknowledge_commands(&mut fake);
    // Without this, the not-waiting gate would explain both silences below by itself (on a real helm the switch
    // clears the mark; the fake keeps it, which is what lets these two gates be tested).
    assert!(
        cx.update(|_, cx| super::reads_waiting("b", cx)),
        "the premise: b still reads as waiting"
    );
    test_support::advance(cx, INTERACTION_GAP);
    scroll_over(cx, "session-row-a");
    assert_eq!(
        interactions(&mut fake),
        Vec::<String>::new(),
        "the list is not the session"
    );

    cx.update(|_, cx| ticker::set_active_override(Some(false), cx));
    assert!(
        cx.update(|_, cx| super::reads_waiting("b", cx)),
        "the premise: b still reads as waiting"
    );
    test_support::click(cx, "main-pane");
    assert_eq!(
        interactions(&mut fake),
        Vec::<String>::new(),
        "an inactive window is not evidence"
    );
}

/// A burst of input (a trackpad's scroll, steady typing) must cost almost nothing, yet an interaction must not be lost
/// to the throttle, or a held alert would go out although the user was looking; the interaction happened in an active
/// window, so the window going inactive before the gap ends must not drop it either. Specified: within one gap, the
/// first of several wheel steps sends at once and the rest, 300 ms apart, send nothing; the window then goes inactive,
/// and when the gap ends exactly one more goes, reporting the age of the latest held-back step (not the first) so the
/// helm dates the attention by it; with nothing further, no more go.
#[gpui::test]
fn the_interaction_signal_is_throttled_but_not_lost(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, &[session("a").blocked()], true);
    scroll_over(cx, "main-pane");
    assert_eq!(interactions(&mut fake).len(), 1, "the first goes at once");
    // Spaced apart, so the age the trailing signal reports tells the latest held-back step from the first.
    let step = std::time::Duration::from_millis(300);
    for _ in 0..3 {
        test_support::advance(cx, step);
        scroll_over(cx, "main-pane");
    }
    assert_eq!(
        interactions(&mut fake),
        Vec::<String>::new(),
        "held for the gap"
    );
    cx.update(|_, cx| ticker::set_active_override(Some(false), cx));
    // The gap runs from the first signal; the latest held-back step came `step * 3` into it.
    test_support::advance(cx, INTERACTION_GAP - step * 3);
    let latest_age = u64::try_from((INTERACTION_GAP - step * 3).as_millis()).unwrap();
    assert_eq!(
        aged(&mut fake),
        [("/api/sessions/a/attention".to_owned(), Some(latest_age))],
        "one trailing signal at the end of the gap, aged from the latest held-back step"
    );
    test_support::advance(cx, INTERACTION_GAP * 3);
    assert_eq!(
        interactions(&mut fake),
        Vec::<String>::new(),
        "nothing more"
    );
}

/// The helm refuses an attention older than `MAX_ATTENTION_AGE_MS`, so a trailing signal, at most about one gap old,
/// must stay well inside it or the throttle would lose interactions after all. Specified: ten gaps fit inside the limit.
#[test]
fn a_trailing_signals_age_stays_inside_what_the_helm_accepts() {
    let limit = std::time::Duration::from_millis(u64::from(farhelm_proto::MAX_ATTENTION_AGE_MS));
    assert!(INTERACTION_GAP * 10 <= limit);
}

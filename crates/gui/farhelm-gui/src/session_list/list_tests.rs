//! Tests of the virtualized rows: that a frame draws only the rows in view, that every row is still reachable by
//! scrolling, and that the things virtualization could break stay as they were: the scroll position across a pushed
//! change, a row's height when its content grows, focus on a row that scrolls out of view, and the Delete shortcut's
//! question for an open row the user cannot see.
//!
//! Like the list's other tests, these run the real main view over the helm client's fake and read rows from GPUI's
//! driver record of the frame, which holds only what was drawn. Rows are found by their open control's id
//! (`session-row-<key>`) and a row's menu button by `session-menu-<key>`. The fleets are idle sessions, so no running
//! pulse animates while a test reads frames.

use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Role, ScrollDelta, ScrollEvent,
    TestAppContext, VisualTestContext, point, px,
};
use serde_json::json;

use super::row::ROW_ID_PREFIX;
use super::selection;
use super::tests::{
    Fixture, change, frame, listing, listing_meta, open_main, preferences, selected, session,
};
use crate::actions::DELETE_SESSION_CHORD;
use crate::test_support;

/// `n` idle sessions keyed `s000`, `s001`, ... (the listing's order), so the keys sort as the order does and no row
/// pulses.
fn fleet(n: usize) -> Vec<Fixture> {
    (0..n)
        .map(|index| {
            // The fixture keeps a `&'static str` key, as every fixed key in these tests is; a test leaks a few.
            let key: &'static str = Box::leak(format!("s{index:03}").into_boxed_str());
            session(key).status(json!({"state": "idle"}))
        })
        .collect()
}

/// The keys of `rows`, in order.
fn keys(rows: &[Fixture]) -> Vec<&'static str> {
    rows.iter().map(|row| row.key).collect()
}

/// The keys of the rows a fresh frame drew, top to bottom (rows past the list's edges included: the list draws a few
/// beyond its view).
fn drawn_rows(cx: &mut VisualTestContext) -> Vec<String> {
    let mut rows: Vec<_> = frame(cx)
        .into_iter()
        .filter(|node| node.role == Some(Role::Button))
        .filter_map(|node| {
            let key = node.element_id.as_deref()?.strip_prefix(ROW_ID_PREFIX)?;
            Some((f32::from(node.bounds.origin.y), key.to_owned()))
        })
        .collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    rows.into_iter().map(|(_, key)| key).collect()
}

/// The id of the element that holds focus, from a fresh frame's driver record.
fn focused(cx: &mut VisualTestContext) -> Option<String> {
    frame(cx)
        .into_iter()
        .find(|node| node.flags.focused && node.element_id.is_some())
        .and_then(|node| node.element_id.map(|id| id.to_string()))
}

/// Scroll the list down by `by` with the pointer over it, as a wheel or trackpad would.
fn scroll_list(cx: &mut VisualTestContext, by: Pixels) {
    let at = test_support::center(cx, "session-rows");
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_event(ScrollEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(0.), -by)),
        ..Default::default()
    });
    test_support::settle(cx);
}

/// The 200-row measurement on the test Mac (2026-11-14) found every window redraw building every row, about 20 ms of
/// CPU per frame at 200 rows, because the pulse and the cursor blink redraw the window several times a second. What
/// virtualization promises is that a frame builds the rows in view and a few past its edges, whatever the fleet's
/// size, and that the list is still whole: SPEC.md has the list "served and rendered WHOLE", so every row must be
/// reachable. Rows the list has not drawn count at an estimated height, so one long scroll can stop a row or so short
/// of the end; the next scroll, as a user would make, reaches it. Specified: with 200 sessions the first frame draws
/// s000 first and fewer than 40 rows; after at most three long scrolls down, the frame draws s199 last and no longer
/// draws s000.
#[gpui::test]
fn only_the_rows_in_view_are_drawn_and_scrolling_reaches_every_row(cx: &mut TestAppContext) {
    let rows = fleet(200);
    let (_fake, cx) = open_main(
        cx,
        vec![listing(&keys(&rows), &rows), preferences(&["s000"], None)],
    );
    let drawn = drawn_rows(cx);
    assert_eq!(drawn.first().map(String::as_str), Some("s000"));
    assert!(
        !drawn.is_empty() && drawn.len() < 40,
        "{} rows drawn of 200",
        drawn.len()
    );

    let mut drawn = Vec::new();
    for _ in 0..3 {
        scroll_list(cx, px(100_000.));
        drawn = drawn_rows(cx);
        if drawn.last().map(String::as_str) == Some("s199") {
            break;
        }
    }
    assert_eq!(drawn.last().map(String::as_str), Some("s199"));
    assert!(!drawn.iter().any(|key| key == "s000"));
}

/// The helm re-sends the listing whenever the fleet changes, and under "Recently active" running sessions reorder it
/// all the time; the list state is spliced on each such change. A splice that dropped the scroll position would throw
/// the user back to the top on every push. The old page kept its scroll offset; the port keeps the same row index and
/// offset. Specified: scrolled down so the first drawn row is row i, a pushed listing with a new session at its top
/// still draws row i of the new order first, and does not draw the new session.
#[gpui::test]
fn a_change_to_the_listing_keeps_the_scroll_position(cx: &mut TestAppContext) {
    let rows = fleet(200);
    let order = keys(&rows);
    let (mut fake, cx) = open_main(
        cx,
        vec![listing(&order, &rows), preferences(&["s000"], None)],
    );
    scroll_list(cx, px(3_000.));
    let before = drawn_rows(cx);
    let first = order
        .iter()
        .position(|key| Some(*key) == before.first().map(String::as_str))
        .unwrap();
    assert!(first > 0, "the premise: the list is scrolled down");

    let mut grown = vec!["new"];
    grown.extend(&order);
    fake.push(change(
        listing_meta(&grown, 201, false),
        &[session("new").status(json!({"state": "idle"}))],
        &[],
    ));
    test_support::settle(cx);
    let after = drawn_rows(cx);
    assert!(!after.iter().any(|key| key == "new"), "{after:?}");
    assert_eq!(
        after.first().map(String::as_str),
        Some(grown[first]),
        "the same row index stays first"
    );
}

/// GPUI's list draws a row only while it is near its view, and a control's focus handle kept in its element state ends
/// with the row's last frame (SPEC_impl.md's cookbook trap), which would throw focus home to the terminal while the
/// user's focus was on the row. Each row's item carries a handle of the list's that contains its controls' focus, and
/// the list state keeps drawing the row whose handle contains focus. Specified: after s000's menu is opened and closed
/// with Escape, focus is on s000's menu button; scrolled far down by the wheel, it still is, and the focus invariant
/// holds.
#[gpui::test]
fn a_rows_menu_button_keeps_focus_when_its_row_scrolls_out_of_view(cx: &mut TestAppContext) {
    let rows = fleet(200);
    let (_fake, cx) = open_main(
        cx,
        vec![listing(&keys(&rows), &rows), preferences(&["s001"], None)],
    );
    test_support::click(cx, "session-menu-s000");
    test_support::keys(cx, "escape");
    assert_eq!(
        focused(cx).as_deref(),
        Some("session-menu-s000"),
        "the premise: focus is on s000's menu button"
    );

    scroll_list(cx, px(5_000.));
    assert!(
        drawn_rows(cx).iter().any(|key| key == "s100"),
        "the premise: the list scrolled far down"
    );
    assert_eq!(focused(cx).as_deref(), Some("session-menu-s000"));
}

/// SPEC.md asks the Delete shortcut's question beside the open session's row, and a question beside a row nobody can
/// see points at nothing. The list builds the rows just below its view to measure them without painting them, so such a
/// row's anchor is recorded, still holding where the row was last painted; the question is asked beside the row only
/// while the row is in the list's view, and centered otherwise. Specified: with the list scrolled so the open session
/// is the last row painted, then scrolled back up by one row so that row sits just below the view and is no longer
/// painted, the shortcut asks a question whose center is the window's horizontal center.
#[gpui::test]
fn the_delete_shortcut_asks_about_an_open_row_out_of_view_in_the_windows_center(
    cx: &mut TestAppContext,
) {
    let rows = fleet(200);
    let (_fake, cx) = open_main(
        cx,
        vec![listing(&keys(&rows), &rows), preferences(&["s000"], None)],
    );
    let row_height = test_support::bounds(cx, "session-row-s001").top()
        - test_support::bounds(cx, "session-row-s000").top();
    scroll_list(cx, px(1_000.));
    let target = drawn_rows(cx).last().cloned().unwrap();
    cx.update(|_, cx| selection::choose(&target, cx));
    test_support::settle(cx);
    assert_eq!(
        selected(cx),
        Some(target.clone()),
        "the premise: {target} is open"
    );

    scroll_list(cx, -row_height);
    assert!(
        !drawn_rows(cx).contains(&target),
        "the premise: {target} is just below the list's view, no longer painted"
    );
    test_support::keys(cx, DELETE_SESSION_CHORD);
    let question = test_support::bounds(cx, "prompt");
    let width = cx.update(|window, _| window.viewport_size().width);
    assert!(
        (question.center().x - width / 2.).abs() < px(2.),
        "centered: {question:?} in a window {width:?} wide"
    );
}

/// SPEC.md, "Reaching everything by keyboard": every scrollable region can be reached and scrolled with the keyboard
/// alone. GPUI's Tab order holds only what was painted, so with only the rows in view painted, Tab from the last of
/// them would leave the list (the review gate saw it go to the terminal). The list scrolls a focused row's neighbors
/// into view. Specified: with 200 sessions and focus on s000's menu button, sixty presses of Tab (every control is a
/// stop in these tests, as on Linux) leave focus on a control of a row at or past s029, far below the first frame's
/// painted rows, and it is painted.
#[gpui::test]
fn tab_walks_past_the_painted_rows(cx: &mut TestAppContext) {
    let rows = fleet(200);
    let (_fake, cx) = open_main(
        cx,
        vec![listing(&keys(&rows), &rows), preferences(&["s001"], None)],
    );
    let first_frame = drawn_rows(cx);
    test_support::click(cx, "session-menu-s000");
    test_support::keys(cx, "escape");
    assert_eq!(
        focused(cx).as_deref(),
        Some("session-menu-s000"),
        "the premise: focus is on s000's menu button"
    );

    for _ in 0..60 {
        test_support::keys(cx, "tab");
    }
    let focused = focused(cx).unwrap_or_default();
    let key = focused
        .strip_prefix(ROW_ID_PREFIX)
        .or_else(|| focused.strip_prefix("session-menu-"))
        .unwrap_or_else(|| panic!("focus left the rows: {focused}"));
    let index: usize = key[1..].parse().unwrap();
    assert!(
        index >= 29 && !first_frame.iter().any(|drawn| drawn == key),
        "focus reached {key}; the first frame painted {first_frame:?}"
    );
    assert!(
        drawn_rows(cx).iter().any(|drawn| drawn == key),
        "{key} is painted"
    );
}

/// A row's menu button keeps focus through a wheel scroll that takes its row out of view, and a press on it must still
/// do what it says beside a row the user can see (the review gate saw Space do nothing there). The row is scrolled back
/// before its menu opens. Specified: with focus on s000's menu button and the list scrolled far down (s000 still drawn,
/// below the window, because it holds focus), Space opens the row's menu and s000's row is back inside the window.
#[gpui::test]
fn a_press_on_a_scrolled_away_menu_button_brings_its_row_back_and_opens_the_menu(
    cx: &mut TestAppContext,
) {
    let rows = fleet(200);
    let (_fake, cx) = open_main(
        cx,
        vec![listing(&keys(&rows), &rows), preferences(&["s001"], None)],
    );
    test_support::click(cx, "session-menu-s000");
    test_support::keys(cx, "escape");
    scroll_list(cx, px(5_000.));
    assert!(
        !drawn_rows(cx).iter().any(|key| key == "s001"),
        "the premise: the rows around s000 are scrolled away"
    );
    assert_eq!(focused(cx).as_deref(), Some("session-menu-s000"));

    let view = cx.update(|window, _| window.viewport_size().height);
    assert!(
        test_support::bounds(cx, "session-row-s000").top() >= view,
        "the premise: s000 is drawn, but below the window"
    );

    test_support::keys(cx, "space");
    assert!(
        cx.debug_bounds("session-menu").is_some(),
        "the menu is open"
    );
    assert!(
        test_support::bounds(cx, "session-row-s000").bottom() <= view,
        "s000 is back in view"
    );
}

/// A click on a row's control focuses it on Linux, and focus moving into a row is what scrolls its neighbors into view
/// for the keyboard. Scrolling on a click's press would move the row away before the release, which then lands on
/// something else, and the click is lost (the review gate reproduced it with a frame between press and release, which
/// the bridge never leaves). Only keyboard focus moves scroll. Specified: pressing the last row painted, drawing a
/// frame, then releasing at the same point opens that session, and the row did not move in between.
#[gpui::test]
fn a_click_on_an_edge_row_opens_it_without_scrolling(cx: &mut TestAppContext) {
    let rows = fleet(200);
    let (_fake, cx) = open_main(
        cx,
        vec![listing(&keys(&rows), &rows), preferences(&["s000"], None)],
    );
    let edge = drawn_rows(cx).last().cloned().unwrap();
    // The bounds helper takes a static selector, as every test's fixed selector is; a test leaks this one.
    let selector: &'static str = Box::leak(format!("{ROW_ID_PREFIX}{edge}").into_boxed_str());
    let before = test_support::bounds(cx, selector);
    let at = point(before.center().x, before.top() + px(4.));
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    test_support::redraw(cx);
    // The premise the test exists for holds only where a click focuses controls; on macOS the press focuses nothing,
    // so the row stays put with or without the keyboard-only rule and the test checks only that the click opens it.
    if crate::focus::CLICK_FOCUSES_CONTROLS {
        assert_eq!(
            focused(cx).as_deref(),
            Some(selector),
            "the premise: the press focused the row's control"
        );
    }
    assert_eq!(
        test_support::bounds(cx, selector).top(),
        before.top(),
        "the row stays under the pointer between press and release"
    );
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    test_support::settle(cx);
    assert_eq!(selected(cx), Some(edge));
}

/// A Tab onto a row already fully in view must not move the list: GPUI's reveal scrolls even a visible row, which
/// snapped a slightly scrolled list back (round two of the review). Specified: with the list scrolled 30 pixels, the
/// rows do not move when Escape returns focus from s002's menu to its menu button, nor when a Tab then moves focus on
/// within the view. Both are keyboard focus moves into a fully visible row.
#[gpui::test]
fn a_tab_within_the_view_does_not_scroll(cx: &mut TestAppContext) {
    let rows = fleet(200);
    let (_fake, cx) = open_main(
        cx,
        vec![listing(&keys(&rows), &rows), preferences(&["s001"], None)],
    );
    scroll_list(cx, px(30.));
    // Read before the menu opens: Escape out of it is itself a keyboard focus move, and a reveal on it would already
    // snap the list back and leave the Tab nothing to move.
    let before = test_support::bounds(cx, "session-row-s004").top();
    test_support::click(cx, "session-menu-s002");
    test_support::keys(cx, "escape");
    assert_eq!(
        focused(cx).as_deref(),
        Some("session-menu-s002"),
        "the premise: focus is on s002's menu button"
    );
    assert_eq!(
        test_support::bounds(cx, "session-row-s004").top(),
        before,
        "Escape into a visible row does not move the list"
    );
    test_support::keys(cx, "tab");
    assert_eq!(
        test_support::bounds(cx, "session-row-s004").top(),
        before,
        "a Tab within the view does not move the list"
    );
}

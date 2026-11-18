//! Tests of dragging a pinned row by its grip in the real main view: which rows have a grip and what it is called, the
//! lifted row and the drop line while the pointer moves, the one request a drop sends, everything that cancels a drag
//! with nothing sent (Escape, a release the window never saw, a release outside the pinned group, the row going away, a
//! failed listing, the window going inactive), and the order hold: showing the moved order at once, and kept through a
//! drag longer than its 5 seconds of stillness; and focus, which the press gives the grip, as the old one took it, so
//! the app's shortcuts still act during a drag, while Escape anywhere in the sidebar's body ends it.
//!
//! Everything runs over the helm client's fake, as the pins' own tests do (whose fixtures and helpers these share), and
//! reads what the window sent from the fake and what it drew from GPUI's driver record and the drag's debug selectors:
//! a row's grip is `session-pin-grip-<key>` (its reveal `session-pin-grip-<key>:shown` or `:hidden`), a lifted row
//! `session-pin-lifted-<key>`, and the drop line `session-pin-drop-<key>:before` or `:after`.
//!
//! The pointer is driven as the platform delivers it: a press, moves with the primary button held, a release. The press
//! takes the order hold, so a drop always lands with the hold on, as it does for a user.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::Availability;
use gpui::{
    Bounds, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px,
};
use serde_json::{Value, json};

use super::menu_tests::any_layer;
use super::menu_tests::{drawn, label_of, push, snapshot};
use super::pin_drag::{GRIP_HELP, GRIP_LABEL};
use super::pins_tests::away;
use super::tests::{change, listing_meta, open_main, row_order, session};
use crate::actions::RENAME_SESSION_CHORD;
use crate::test_support::{self, focused_id, frame};

// ---- Fixtures and helpers ------------------------------------------------------------------------------------------

/// Every command the window sent since the last call, answered with success in order, as (method, path, body): all of
/// them, so a test that expects one move and nothing else sees anything else the drag might have sent.
fn sent(fake: &mut FakeHelm) -> Vec<(Method, String, Value)> {
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

/// `PUT /api/sessions/<key>/pin-order` naming `neighbor` and `placement`.
fn move_put(key: &str, neighbor: &str, placement: &str) -> (Method, String, Value) {
    (
        Method::Put,
        format!("/api/sessions/{key}/pin-order"),
        json!({ "neighbor": neighbor, "placement": placement, "areas": { "source": "ungrouped", "destination": "ungrouped" } }),
    )
}

/// Where the element with id `id` was drawn in a fresh frame. Panics when it was not, since the caller's next step would
/// act on a point that is not the element.
fn bounds_of(cx: &mut VisualTestContext, id: &str) -> Bounds<Pixels> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("{id} was not drawn"))
        .bounds
}

/// `key`'s row item, the whole row a drop is judged against.
fn item(cx: &mut VisualTestContext, key: &str) -> Bounds<Pixels> {
    bounds_of(cx, &format!("session-item-{key}"))
}

/// Press the primary button on the middle of `key`'s grip, and let the window settle.
fn press_grip(cx: &mut VisualTestContext, key: &str) -> Point<Pixels> {
    let at = bounds_of(cx, &format!("session-pin-grip-{key}")).center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    test_support::settle(cx);
    at
}

/// Move the pointer to `at` with the primary button held, and let the window settle.
fn drag_to(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_move(at, Some(MouseButton::Left), Modifiers::none());
    test_support::settle(cx);
}

/// Release the primary button at `at`, and let the window settle.
fn release(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    test_support::settle(cx);
}

/// A point 3 pixels inside the top of `key`'s row, halfway across: above its middle, so a drop there goes before it.
fn near_top(cx: &mut VisualTestContext, key: &str) -> Point<Pixels> {
    let row = item(cx, key);
    point(row.center().x, row.top() + px(3.))
}

/// Whether any of `keys`' rows shows lifted or draws a drop line: what every cancellation must leave behind. GPUI looks
/// debug selectors up by `&'static str`, so the selectors built here are leaked, a few short strings per test process.
fn drag_painted(cx: &mut VisualTestContext, keys: &[&'static str]) -> bool {
    keys.iter().any(|key| {
        let lifted: &'static str = format!("session-pin-lifted-{key}").leak();
        let before: &'static str = format!("session-pin-drop-{key}:before").leak();
        let after: &'static str = format!("session-pin-drop-{key}:after").leak();
        drawn(cx, lifted) || drawn(cx, before) || drawn(cx, after)
    })
}

/// The element id of `key`'s grip, the one control that holds focus while the grip does (`focused_id` reports it).
fn grip_focus(key: &str) -> Option<String> {
    Some(format!("session-pin-grip-{key}"))
}

/// a, b and c pinned at 1, 2 and 3, then u and v unpinned, served in that order with a open.
fn three_pins() -> Vec<farhelm_proto::ui_channel::CollectionState> {
    snapshot(
        &[
            (session("a").pinned(1), 0),
            (session("b").pinned(2), 0),
            (session("c").pinned(3), 0),
            (session("u"), 0),
            (session("v"), 0),
        ],
        false,
    )
}

// ---- The grip ------------------------------------------------------------------------------------------------------

/// SPEC.md, "Session list": "Hovering an eligible pinned row reveals a grip beside its name", and "A newly pinned row
/// waiting for the order hold to end has neither a grip nor Move actions"; the old grip was a button named "Drag pinned
/// session" over the row's leading gutter, on movable pins only, shown on hover. Its help is checked beside the sidebar
/// with the other row marks' (`summary_tests.rs`, `every_row_mark_shows_its_help_beside_the_sidebar_with_its_ring`).
/// Specified: with a and b pinned and u unpinned, a's and b's grips are drawn, named "Drag pinned session", at the top
/// left of their rows, 12 by 28 pixels, hidden until the pointer is over the row, and u has none; c pinned by the helm
/// while the pointer is over the rows (held third) has no grip until the pointer leaves, when it leads and has one.
#[gpui::test]
fn movable_pins_have_a_named_grip_revealed_by_the_pointer(cx: &mut TestAppContext) {
    let rows = [
        (session("a").pinned(1), 0),
        (session("b").pinned(2), 0),
        (session("u"), 0),
        (session("c"), 0),
    ];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    away(cx);
    assert_eq!(row_order(&frame(cx)), ["a", "b", "u", "c"], "the premise");
    assert_eq!(GRIP_HELP, "Drag to reorder pinned sessions");
    for key in ["a", "b"] {
        let id = format!("session-pin-grip-{key}");
        assert_eq!(label_of(cx, &id).as_deref(), Some(GRIP_LABEL));
        let node = frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(id.as_str()));
        assert_eq!(
            node.and_then(|node| node.role),
            Some(gpui::Role::Button),
            "{key}: a button"
        );
        assert_eq!(GRIP_LABEL, "Drag pinned session");
        let (grip, row) = (bounds_of(cx, &id), item(cx, key));
        assert_eq!(grip.origin, row.origin, "{key}: at the row's top left");
        assert_eq!(grip.size, gpui::size(px(12.), px(28.)));
    }
    assert!(drawn(cx, "session-pin-grip-a:hidden") && drawn(cx, "session-pin-grip-b:hidden"));
    assert!(!drawn(cx, "session-pin-grip-u:hidden") && !drawn(cx, "session-pin-grip-u:shown"));

    let over_b = item(cx, "b").center();
    cx.simulate_mouse_move(over_b, None, Modifiers::none());
    test_support::settle(cx);
    assert!(
        drawn(cx, "session-pin-grip-b:shown"),
        "revealed by the pointer"
    );
    assert!(drawn(cx, "session-pin-grip-a:hidden"));

    push(
        &mut fake,
        &["a", "b", "u", "c"],
        &[(session("c").pinned(0), 0)],
        cx,
    );
    assert_eq!(row_order(&frame(cx)), ["a", "b", "u", "c"], "held");
    assert!(drawn(cx, "session-row-c-pin"), "the premise: c is pinned");
    assert!(!drawn(cx, "session-pin-grip-c:hidden") && !drawn(cx, "session-pin-grip-c:shown"));
    away(cx);
    assert_eq!(row_order(&frame(cx)), ["c", "a", "b", "u"]);
    assert!(drawn(cx, "session-pin-grip-c:hidden"));
}

// ---- A drop --------------------------------------------------------------------------------------------------------

/// SPEC.md: "Dragging that grip within the pinned group shows the row lifted and a drop line; release moves it relative
/// to the pin at that line", and a manual move "takes effect without waiting for the hold to end". The old
/// `pins.spec.ts` case "drag pins within their group and cancel interrupted moves", its successful drop: the paint must
/// not reorder the rows, and the release sends one relative move; the press focused the grip, and the old grip, disabled
/// while its move was in flight, lost focus to the page. Specified: with a, b, c pinned and u, v unpinned, and focus
/// where the window put it, a press on c's grip gives the grip focus, and it and a 2-pixel move lift nothing; a move to
/// 3 pixels inside a's top lifts c and draws the drop line inside a's top edge, 2 pixels high, with the rows still a, b,
/// c, u, v and nothing sent; the release sends exactly `{"neighbor": "a", "placement": "before"}` to c's pin-order route
/// and nothing else, takes the lift and the line away, draws no grip on c while the move is in flight, and focus goes
/// back where it rested before the press; the
/// helm's new positions then show as c, a, b with the pointer still over the rows, while u and v keep their held slots
/// against a push that serves v first, until the pointer leaves.
#[gpui::test]
fn a_drop_on_the_drop_line_sends_one_move_that_the_hold_shows_at_once(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, three_pins());
    away(cx);
    sent(&mut fake);
    let focused = focused_id(cx);
    let all = ["a", "b", "c", "u", "v"];
    assert_eq!(row_order(&frame(cx)), all, "the premise");

    let pressed = press_grip(cx, "c");
    assert_eq!(
        focused_id(cx),
        grip_focus("c"),
        "the press focuses the grip"
    );
    drag_to(cx, pressed + point(px(0.), px(-2.)));
    assert!(
        !drag_painted(cx, &all),
        "no lift before the pointer moves 4 pixels"
    );

    let target = near_top(cx, "a");
    drag_to(cx, target);
    assert!(drawn(cx, "session-pin-lifted-c"));
    let (line, row) = (
        test_support::bounds(cx, "session-pin-drop-a:before"),
        item(cx, "a"),
    );
    assert_eq!((line.top(), line.size.height), (row.top(), px(2.)));
    assert!(!drawn(cx, "session-pin-drop-a:after") && !drawn(cx, "session-pin-drop-b:before"));
    assert_eq!(
        row_order(&frame(cx)),
        all,
        "the paint does not reorder the rows"
    );
    assert!(
        sent(&mut fake).is_empty(),
        "nothing is sent before the release"
    );

    cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    test_support::settle(cx);
    let [command] = &fake.commands()[..] else {
        panic!("exactly one request");
    };
    let pending = command.id;
    assert_eq!(
        (
            command.method,
            command.path.clone(),
            command.body.clone().unwrap_or_default()
        ),
        move_put("c", "a", "before")
    );
    assert!(!drag_painted(cx, &all));
    assert!(
        !drawn(cx, "session-pin-grip-c:shown") && !drawn(cx, "session-pin-grip-c:hidden"),
        "no grip while the move is in flight"
    );
    assert_eq!(focused_id(cx), focused, "the lost focus went home");

    fake.reply(pending, 200, None);
    push(
        &mut fake,
        &["v", "u", "a", "b", "c"],
        &[
            (session("c").pinned(1), 0),
            (session("a").pinned(2), 0),
            (session("b").pinned(3), 0),
        ],
        cx,
    );
    assert_eq!(
        row_order(&frame(cx)),
        ["c", "a", "b", "u", "v"],
        "the hold lets the move through and keeps the rest"
    );
    assert!(drawn(cx, "session-pin-grip-c:shown") || drawn(cx, "session-pin-grip-c:hidden"));
    away(cx);
    assert_eq!(row_order(&frame(cx)), ["c", "a", "b", "v", "u"]);
    assert!(sent(&mut fake).is_empty(), "and nothing more was sent");
}

/// The shared grip uses the header's bottom edge when a group has no
/// eligible pins, even if its former pin is still shown in a held slot.
/// Only the initiating window's successful reply moves the source slot.
#[gpui::test]
fn an_empty_group_accepts_its_first_pin_and_moves_the_held_slot(cx: &mut TestAppContext) {
    use super::groups_tests::{group, grouped_snapshot};
    let roster = vec![group(10, "Work", 2, false), group(20, "Other", 1, true)];
    let order = ["global", "member", "old", "other"];
    let state = grouped_snapshot(
        &[
            session("global").pinned(1),
            session("member").grouped(10),
            session("old").pinned(3).grouped(10),
            session("other").grouped(20),
        ],
        roster,
    );
    let (mut fake, cx) = open_main(cx, state);
    away(cx);
    sent(&mut fake);
    let header = |cx: &mut VisualTestContext| bounds_of(cx, "session-group-10");
    assert!(item(cx, "global").bottom() <= header(cx).top());
    press_grip(cx, "global");
    let mut meta = listing_meta(&order, 4, false);
    meta.session_groups = vec![group(10, "Work", 1, false), group(20, "Other", 2, true)];
    fake.push(change(meta, &[session("old").pinned(3).grouped(20)], &[]));
    test_support::settle(cx);
    assert_eq!(
        row_order(&frame(cx)),
        ["global", "old", "member"],
        "the old pin still occupies its held slot"
    );
    let bounds = header(cx);
    let target = point(bounds.center().x, bounds.bottom() - px(3.));
    drag_to(cx, target);
    assert!(drawn(cx, "session-pin-drop-group/10:after"));
    release(cx, target);
    let [command] = &fake.commands()[..] else {
        panic!("one move")
    };
    assert_eq!(command.path, "/api/sessions/global/group");
    assert_eq!(
        command.body,
        Some(json!({"target": {"group": 10}, "pin_source": "ungrouped"}))
    );
    let mut meta = listing_meta(&order, 4, false);
    meta.session_groups = vec![group(10, "Work", 2, false), group(20, "Other", 2, true)];
    fake.push(change(
        meta,
        &[session("global").pinned(2).grouped(10)],
        &[],
    ));
    test_support::settle(cx);
    assert!(
        item(cx, "global").bottom() <= header(cx).top(),
        "a membership push alone keeps the held slot"
    );
    fake.reply(command.id, 200, None);
    test_support::settle(cx);
    assert!(
        header(cx).bottom() <= item(cx, "global").top(),
        "the completed drop moves the pin into its group"
    );
}

/// The group header grip uses the pin gesture's line and relative request
/// while accepting other headers as neighbors.
#[gpui::test]
fn a_group_drag_names_the_header_neighbor(cx: &mut TestAppContext) {
    use super::groups_tests::{group, grouped_snapshot};
    let state = grouped_snapshot(
        &[session("a").grouped(10), session("b").grouped(20)],
        vec![group(10, "First", 1, true), group(20, "Second", 1, true)],
    );
    let (mut fake, cx) = open_main(cx, state);
    sent(&mut fake);
    assert_eq!(
        label_of(cx, "session-pin-grip-group/20").as_deref(),
        Some("Drag group")
    );
    press_grip(cx, "group/20");
    let first = bounds_of(cx, "session-group-10");
    let target = point(first.center().x, first.top() + px(3.));
    drag_to(cx, target);
    assert!(drawn(cx, "session-pin-drop-group/10:before"));
    release(cx, target);
    assert_eq!(
        sent(&mut fake),
        vec![(
            Method::Put,
            "/api/session-groups/20/order".into(),
            json!({"neighbor": 10, "placement": "before"})
        )]
    );
}

/// The other side of the drop line: a release below the last pin's middle puts the dragged pin after it, the route's
/// other placement. Specified: with a, b, c pinned, a's grip dragged to 3 pixels above c's bottom draws the line inside
/// c's bottom edge and lifts a; the release sends `{"neighbor": "c", "placement": "after"}` to a's pin-order route.
#[gpui::test]
fn a_drop_below_the_last_pins_middle_goes_after_it(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, three_pins());
    away(cx);
    sent(&mut fake);
    press_grip(cx, "a");
    let row = item(cx, "c");
    let target = point(row.center().x, row.bottom() - px(3.));
    drag_to(cx, target);
    assert!(drawn(cx, "session-pin-lifted-a"));
    let line = test_support::bounds(cx, "session-pin-drop-c:after");
    assert!(
        line.bottom() <= row.bottom() && line.bottom() >= row.bottom() - px(1.),
        "inside c's bottom edge: {line:?} in {row:?}"
    );
    release(cx, target);
    assert_eq!(sent(&mut fake), [move_put("a", "c", "after")]);
}

// ---- Cancelling ----------------------------------------------------------------------------------------------------

/// SPEC.md: "Escape, interrupted pointer capture or releasing outside the pinned group cancels", and "Dragging never
/// ... crosses the separator". The old `pins.spec.ts` case's cancellations, each begun with a real drop line so that a
/// drag that never armed could not pass. Specified, for each of Escape, a move with the button no longer held (a
/// release the window never saw), a move 20 pixels past the rows' right edge, and a move onto the unpinned u below the
/// separator: c's grip pressed and dragged to 3 pixels inside a's top draws the line on a, with focus on the grip;
/// Escape, which the sidebar's body catches on its way down to the focused grip, ends the drag at once; after the
/// cancellation and a release there, no row is lifted, no line is drawn, the rows are a, b, c, u, v, nothing was sent,
/// and focus is still on c's grip, as the old one kept it.
#[gpui::test]
fn escape_a_lost_release_and_a_release_outside_send_nothing(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, three_pins());
    away(cx);
    sent(&mut fake);
    let all = ["a", "b", "c", "u", "v"];
    let rows = test_support::bounds(cx, "session-rows");
    for cancellation in ["escape", "lost", "outside", "separator"] {
        press_grip(cx, "c");
        let target = near_top(cx, "a");
        drag_to(cx, target);
        assert!(
            drawn(cx, "session-pin-drop-a:before"),
            "{cancellation}: the premise, a real drop line"
        );
        assert_eq!(focused_id(cx), grip_focus("c"), "{cancellation}");
        let at = match cancellation {
            "escape" => {
                test_support::keys(cx, "escape");
                test_support::settle(cx);
                assert!(!drag_painted(cx, &all), "Escape ended the drag");
                near_top(cx, "a")
            }
            "lost" => {
                let at = near_top(cx, "a");
                cx.simulate_mouse_move(at, None, Modifiers::none());
                test_support::settle(cx);
                at
            }
            "outside" => {
                let at = point(rows.right() + px(20.), near_top(cx, "a").y);
                drag_to(cx, at);
                at
            }
            _ => {
                let at = item(cx, "u").center();
                drag_to(cx, at);
                at
            }
        };
        if cancellation == "outside" || cancellation == "separator" {
            assert!(
                drawn(cx, "session-pin-lifted-c") && !drag_painted(cx, &["a", "b", "u", "v"]),
                "{cancellation}: still lifted, with no drop line"
            );
        }
        release(cx, at);
        assert!(
            !drag_painted(cx, &all),
            "{cancellation}: nothing left drawn"
        );
        assert_eq!(row_order(&frame(cx)), all, "{cancellation}");
        assert_eq!(
            focused_id(cx),
            grip_focus("c"),
            "{cancellation}: focus stays"
        );
        assert!(sent(&mut fake).is_empty(), "{cancellation}: nothing sent");
        away(cx);
    }
}

/// The old `pins.spec.ts` case's last part: the dragged row deleted under the pointer ends the drag, and the release
/// that follows moves nothing. Specified: with c's grip dragged to a's top (the line on a), the helm removing c takes
/// the lift and the line away; the release then sends nothing, and a and b are still
/// listed, first and second.
#[gpui::test]
fn the_dragged_row_going_away_cancels_the_drag(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, three_pins());
    away(cx);
    sent(&mut fake);
    press_grip(cx, "c");
    let target = near_top(cx, "a");
    drag_to(cx, target);
    assert!(drawn(cx, "session-pin-drop-a:before"), "the premise");
    fake.push(change(
        listing_meta(&["a", "b", "u", "v"], 4, false),
        &[],
        &["c"],
    ));
    test_support::settle(cx);
    assert!(!drag_painted(cx, &["a", "b", "u", "v"]));
    release(cx, target);
    assert!(sent(&mut fake).is_empty());
    assert_eq!(row_order(&frame(cx)), ["a", "b", "u", "v"]);
}

/// The old `pins.spec.ts` case "pin dragging survives a failed listing and replacement list mount": a failed listing
/// replaced the old list's root, taking a drag on it along, and the list that came back dragged as before. Here a failed
/// read keeps the rows on screen, so the drag must end by rule rather than with a root. Specified: with b's grip dragged
/// to a's top (the line on a), a pushed listing the helm could not read keeps every row but draws no grip and ends the
/// drag (no lift, no line); the release sends nothing; once the helm reads the listing again, b's grip
/// is back, and a drag of it to a's top sends `{"neighbor": "a", "placement": "before"}` to b's pin-order route.
#[gpui::test]
fn a_failed_listing_ends_a_drag_and_the_recovered_list_drags_again(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, three_pins());
    away(cx);
    sent(&mut fake);
    let all = ["a", "b", "c", "u", "v"];
    press_grip(cx, "b");
    let target = near_top(cx, "a");
    drag_to(cx, target);
    assert!(drawn(cx, "session-pin-drop-a:before"), "the premise");

    let mut failed = listing_meta(&all, 5, false);
    failed.availability = Availability::Unavailable {
        error: "controlled listing failure".into(),
    };
    fake.push(change(failed, &[], &[]));
    test_support::settle(cx);
    assert_eq!(row_order(&frame(cx)), all, "the rows stay");
    assert!(!drag_painted(cx, &all));
    assert!(!drawn(cx, "session-pin-grip-b:shown") && !drawn(cx, "session-pin-grip-b:hidden"));
    release(cx, target);
    assert!(sent(&mut fake).is_empty());

    fake.push(change(listing_meta(&all, 5, false), &[], &[]));
    test_support::settle(cx);
    away(cx);
    press_grip(cx, "b");
    let target = near_top(cx, "a");
    drag_to(cx, target);
    assert!(drawn(cx, "session-pin-drop-a:before"));
    release(cx, target);
    assert_eq!(sent(&mut fake), [move_put("b", "a", "before")]);
}

/// SPEC.md: "Escape, interrupted pointer capture or releasing outside the pinned group cancels". A window that stops
/// being active mid-drag (another app took the pointer) may never report the release, so the drag ends at once.
/// Specified: in an active window, with c's grip dragged to a's top (the line on a), the window going inactive takes
/// the lift and the line away; the release that follows sends nothing.
#[gpui::test]
fn the_window_going_inactive_ends_a_drag(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, three_pins());
    away(cx);
    sent(&mut fake);
    assert!(
        cx.update(|window, _| window.is_window_active()),
        "the premise: an active window"
    );
    press_grip(cx, "c");
    let target = near_top(cx, "a");
    drag_to(cx, target);
    assert!(drawn(cx, "session-pin-drop-a:before"), "the premise");
    cx.deactivate_window();
    test_support::settle(cx);
    assert!(!drag_painted(cx, &["a", "b", "c", "u", "v"]));
    release(cx, target);
    assert!(sent(&mut fake).is_empty());
}

/// The drag stops every move it captures before the rows' own listeners see it, so it must tell the order hold about
/// each one itself (`pin_drag_moved`); otherwise 5 seconds after the press the hold would let go mid-drag and rows could
/// move under the pointer and the drop line. SPEC.md: "pointer movement over the list starts the 5 seconds of
/// stillness over". Specified: with c's grip dragged to a's top and the helm then serving v before u, the rows stay a,
/// b, c, u, v 4 seconds on; after a move 1 pixel down and 4 seconds more (8 since the press, 4 since the move) they
/// still do; the release sends c's move before a.
#[gpui::test]
fn a_drag_longer_than_the_stillness_keeps_the_order_hold(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, three_pins());
    away(cx);
    sent(&mut fake);
    let all = ["a", "b", "c", "u", "v"];
    press_grip(cx, "c");
    let target = near_top(cx, "a");
    drag_to(cx, target);
    assert!(drawn(cx, "session-pin-drop-a:before"), "the premise");
    fake.push(change(
        listing_meta(&["a", "b", "c", "v", "u"], 5, false),
        &[],
        &[],
    ));
    test_support::settle(cx);
    assert_eq!(row_order(&frame(cx)), all, "the premise: held");

    let four = super::hold::STILLNESS - std::time::Duration::from_secs(1);
    test_support::advance(cx, four);
    assert_eq!(row_order(&frame(cx)), all);
    let moved = target + point(px(0.), px(1.));
    drag_to(cx, moved);
    test_support::advance(cx, four);
    assert_eq!(
        row_order(&frame(cx)),
        all,
        "the drag's move started the 5 seconds over"
    );
    release(cx, moved);
    assert_eq!(sent(&mut fake), [move_put("c", "a", "before")]);
}

/// The old grip took focus on its press, so the app's shortcuts still acted during a drag; the drag must not take the
/// keys. With a dialog open, Escape is the layer manager's: it closes the topmost layer (SPEC_impl.md, "Overlays: the
/// layer manager"), where the old document-level listener ended the drag and left the dialog open. Specified: with c's
/// grip dragged to a's top (the line on a), the Rename shortcut opens the Rename editor for the open session while c
/// stays lifted; Escape then closes the editor, and the drag goes on with its line on a; a release beside the rows
/// sends nothing.
#[gpui::test]
fn the_apps_shortcuts_still_act_during_a_drag(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, three_pins());
    away(cx);
    sent(&mut fake);
    press_grip(cx, "c");
    let target = near_top(cx, "a");
    drag_to(cx, target);
    assert!(drawn(cx, "session-pin-drop-a:before"), "the premise");

    test_support::keys(cx, RENAME_SESSION_CHORD);
    assert!(drawn(cx, "rename"), "the shortcut opened Rename");
    assert!(drawn(cx, "session-pin-lifted-c"), "the drag goes on");
    test_support::keys(cx, "escape");
    test_support::settle(cx);
    assert!(!drawn(cx, "rename"), "Escape closed Rename");
    drag_to(cx, target);
    assert!(drawn(cx, "session-pin-drop-a:before"), "the drag goes on");

    let rows = test_support::bounds(cx, "session-rows");
    let beside = point(rows.right() + px(20.), target.y);
    drag_to(cx, beside);
    release(cx, beside);
    assert!(sent(&mut fake).is_empty());
}

/// SPEC.md: "Escape ... cancels", whatever holds focus, as the old document-level listener had it. A press on the grip
/// with a row's menu open cannot give the grip focus (the coordinator refuses a move out of an open layer, and the
/// closing menu sends focus back to its button), so Escape must reach the drag from there too. Specified: with a's menu
/// open from a click, a press on c's grip closes the menu, leaves focus on a's menu button and starts a drag; dragged to
/// a's top (the line on a), Escape ends the drag and opens nothing; the release sends nothing.
#[gpui::test]
fn escape_ends_a_drag_begun_with_a_rows_menu_open(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, three_pins());
    away(cx);
    sent(&mut fake);
    test_support::click(cx, "session-menu-a");
    assert!(drawn(cx, "session-menu"), "the premise: a's menu is open");
    press_grip(cx, "c");
    let target = near_top(cx, "a");
    drag_to(cx, target);
    assert!(
        drawn(cx, "session-pin-drop-a:before"),
        "the premise: a drag"
    );
    assert!(!any_layer(cx), "the press closed the menu");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-a"));
    test_support::keys(cx, "escape");
    test_support::settle(cx);
    assert!(
        !drag_painted(cx, &["a", "b", "c", "u", "v"]),
        "Escape ended the drag"
    );
    assert!(!any_layer(cx));
    release(cx, target);
    assert!(sent(&mut fake).is_empty());
}

/// The same rule after Tab carried focus off the grip mid-drag, which the old app's capture listener covered too.
/// Specified: with c's grip dragged to a's top (the line on a) and Tab pressed, focus is no longer on the grip and the
/// drag goes on; Escape then ends it; the release sends nothing.
#[gpui::test]
fn escape_ends_a_drag_after_tab_moved_focus_off_the_grip(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, three_pins());
    away(cx);
    sent(&mut fake);
    press_grip(cx, "c");
    let target = near_top(cx, "a");
    drag_to(cx, target);
    assert_eq!(focused_id(cx), grip_focus("c"), "the premise");
    test_support::keys(cx, "tab");
    assert_ne!(focused_id(cx), grip_focus("c"), "Tab moved focus on");
    assert!(drawn(cx, "session-pin-drop-a:before"), "the drag goes on");
    test_support::keys(cx, "escape");
    test_support::settle(cx);
    assert!(
        !drag_painted(cx, &["a", "b", "c", "u", "v"]),
        "Escape ended the drag"
    );
    release(cx, target);
    assert!(sent(&mut fake).is_empty());
}

/// The old listener stopped the Escape that ended a drag, so nothing else acted on that key; the list's own Escape
/// clears the marks, and the root's closes the topmost layer. Specified: with u marked by a Shift-click, c's grip
/// dragged to a's top and Escape pressed, the drag ends and u is still marked; a second Escape, with no drag on, clears
/// the mark as usual.
#[gpui::test]
fn the_escape_that_ends_a_drag_goes_no_further(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, three_pins());
    away(cx);
    sent(&mut fake);
    let at = test_support::center(cx, "session-row-u");
    cx.simulate_click(at, Modifiers::shift());
    test_support::settle(cx);
    assert!(drawn(cx, "session-marked-u"), "the premise: u is marked");
    away(cx);
    press_grip(cx, "c");
    let target = near_top(cx, "a");
    drag_to(cx, target);
    assert!(
        drawn(cx, "session-pin-drop-a:before"),
        "the premise: a drag"
    );
    test_support::keys(cx, "escape");
    test_support::settle(cx);
    assert!(
        !drag_painted(cx, &["a", "b", "c", "u", "v"]),
        "Escape ended the drag"
    );
    assert!(drawn(cx, "session-marked-u"), "and went no further");
    release(cx, target);
    test_support::keys(cx, "escape");
    test_support::settle(cx);
    assert!(
        !drawn(cx, "session-marked-u"),
        "with no drag on, Escape clears the marks"
    );
    assert!(
        sent(&mut fake)
            .iter()
            .all(|(_, path, _)| !path.ends_with("/pin-order"))
    );
}

/// SPEC.md, "Retired containerized sessions": such a session supports Delete only, and the old app disabled a retired
/// row's grip; the row stayed a drop target, and the helm takes it as a neighbor, as another pin's Move up and Move down
/// do. Specified: with a pinned and r, a retired containerized session, pinned below it, and b pinned below r, a's and
/// b's grips are drawn and r has none; b's grip dragged to 3 pixels inside r's top draws the line before r, and the
/// release sends `{"neighbor": "r", "placement": "before"}` to b's pin-order route.
#[gpui::test]
fn a_retired_pin_has_no_grip_but_is_a_drop_target(cx: &mut TestAppContext) {
    let rows = [
        (session("a").pinned(1), 0),
        (super::retired_tests::retired("r").pinned(2), 0),
        (session("b").pinned(3), 0),
        (session("u"), 0),
    ];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    away(cx);
    sent(&mut fake);
    assert_eq!(row_order(&frame(cx)), ["a", "r", "b", "u"], "the premise");
    assert!(drawn(cx, "session-pin-grip-a:hidden") && drawn(cx, "session-pin-grip-b:hidden"));
    assert!(!drawn(cx, "session-pin-grip-r:hidden") && !drawn(cx, "session-pin-grip-r:shown"));

    press_grip(cx, "b");
    let target = near_top(cx, "r");
    drag_to(cx, target);
    assert!(drawn(cx, "session-pin-lifted-b"), "the premise: lifted");
    assert!(drawn(cx, "session-pin-drop-r:before"));
    release(cx, target);
    assert_eq!(sent(&mut fake), [move_put("b", "r", "before")]);
}

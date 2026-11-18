//! Tests of pins and the order hold in the real main view: Pin to top, Unpin, Move up and Move down in a row's menu
//! (their words, where they sit, when they are offered, the exact request each sends, what a pending or refused one
//! does), the order and the separator a pinned list is drawn with, the glyph and the row's accessible name, focus after
//! a Move, and the hold's rules (the pointer over the rows holds the order, leaving or 5 seconds of stillness releases
//! it, an open row menu keeps it, and its exceptions).
//!
//! Everything runs over the helm client's fake, as the menu's own tests do (whose fixtures and helpers these share),
//! and reads what the window sent from the fake and what it drew from GPUI's driver record of the frame. A row's menu
//! button is `session-menu-<key>`, the pin item `session-menu-pin`, the move items `session-menu-move-up` and
//! `session-menu-move-down`, a row's glyph `session-row-<key>-pin`, and the separator `session-pin-separator`.
//!
//! The pointer matters here as nowhere else in the list's tests: a click on a row's control leaves the pointer over the
//! rows, which takes the hold, so every test says where the pointer is before it pushes a new order ([`away`],
//! [`over`]).

use std::time::Duration;

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use gpui::{
    Modifiers, Point, ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext, point, px,
};
use serde_json::{Value, json};

use super::hold::STILLNESS;
use super::menu_tests::{any_layer, drawn, label_of, push, snapshot};
use super::tests::{change, listing_meta, open_main, painted, row_order, session};
use crate::main_view::MainView;
use crate::test_support::{self, focused_id, frame};

// ---- Fixtures and helpers ------------------------------------------------------------------------------------------

/// Every command the window sent since the last call, answered with success in order, as (method, path, body), keeping
/// only the pin routes': the window also sends preference writes, seen marks and session queries, which these tests do
/// not count.
fn pin_requests(fake: &mut FakeHelm) -> Vec<(Method, String, Value)> {
    fake.commands()
        .into_iter()
        .filter_map(|command| {
            fake.reply(command.id, 200, None);
            (command.path.ends_with("/pin") || command.path.ends_with("/pin-order")).then(|| {
                (
                    command.method,
                    command.path,
                    command.body.unwrap_or_default(),
                )
            })
        })
        .collect()
}

/// `PUT /api/sessions/<key>/pin` with `pinned`.
fn pin_put(key: &str, pinned: bool) -> (Method, String, Value) {
    (
        Method::Put,
        format!("/api/sessions/{key}/pin"),
        json!({ "pinned": pinned }),
    )
}

/// `PUT /api/sessions/<key>/pin-order` naming `neighbor` and `placement`.
fn move_put(key: &str, neighbor: &str, placement: &str) -> (Method, String, Value) {
    (
        Method::Put,
        format!("/api/sessions/{key}/pin-order"),
        json!({ "neighbor": neighbor, "placement": placement, "areas": { "source": "ungrouped", "destination": "ungrouped" } }),
    )
}

/// Move the pointer off the rows, onto the session pane beside them, and let the window settle: the hold, if one is
/// on and no row menu keeps it, ends.
pub(super) fn away(cx: &mut VisualTestContext) {
    let rows = test_support::bounds(cx, "session-rows");
    let beside = Point::new(rows.right() + px(200.), rows.center().y);
    cx.simulate_mouse_move(beside, None, Modifiers::none());
    test_support::settle(cx);
}

/// Move the pointer onto the middle of what is drawn under `selector`, a row's control, and let the window settle.
fn over(cx: &mut VisualTestContext, selector: &'static str) {
    let at = test_support::center(cx, selector);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    test_support::settle(cx);
}

/// Open `key`'s menu by a click on its button (`button`) and press Tab until `item` holds focus, as a keyboard user
/// walks the menu. Panics when the menu does not offer it.
fn focus_item(cx: &mut VisualTestContext, button: &'static str, item: &str) {
    test_support::click(cx, button);
    for _ in 0..8 {
        if focused_id(cx).as_deref() == Some(item) {
            return;
        }
        test_support::keys(cx, "tab");
    }
    panic!("{item} is not in the menu");
}

/// The rows in the helm's order `order`, pushed as a complete listing.
fn reorder(fake: &mut FakeHelm, order: &[&str], cx: &mut VisualTestContext) {
    fake.push(change(
        listing_meta(order, order.len() as u64, false),
        &[],
        &[],
    ));
    test_support::settle(cx);
}

/// Whether the separator is drawn between the rows drawn under `above` and `below`.
fn separator_between(cx: &mut VisualTestContext, above: &'static str, below: &'static str) -> bool {
    let Some(line) = cx.debug_bounds("session-pin-separator") else {
        return false;
    };
    let (above, below) = (
        test_support::bounds(cx, above),
        test_support::bounds(cx, below),
    );
    line.top() >= above.bottom() && line.bottom() <= below.top()
}

// ---- Pin to top and Unpin ------------------------------------------------------------------------------------------

/// SPEC.md, "Session list": "Directly after it in the menu's first metadata group, **Pin to top** / **Unpin** toggles
/// the shared pin", pinned rows come first, "A muted pin glyph immediately before the title names a pinned row in its
/// tooltip and accessible text", and the separator follows the leading pinned run "only when an unpinned row follows".
/// The old `pins.spec.ts` case "pin from the menu groups the sidebar and unpin restores title order". Specified: with
/// a, b and c unpinned in that order and a open, c's menu offers "Pin to top" with its description right after Mark
/// unread; Enter on it closes the menu, leaves focus on c's button and sends `{"pinned": true}` to c's pin route; once
/// the helm pushes c pinned (pointer off the rows), c leads, draws a glyph named "Pinned", reads "Pinned c" as its name,
/// and the separator sits between c and a; c's menu then offers "Unpin" and, as a lone pin, no Move items; Unpin sends
/// `{"pinned": false}`, and once pushed the list is a, b, c again with no glyph and no separator.
#[gpui::test]
fn pin_and_unpin_regroup_the_list_with_the_glyph_and_the_separator(cx: &mut TestAppContext) {
    let rows = [(session("a"), 0), (session("b"), 0), (session("c"), 0)];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    pin_requests(&mut fake);
    assert_eq!(row_order(&frame(cx)), ["a", "b", "c"], "the premise");

    focus_item(cx, "session-menu-c", "session-menu-pin");
    assert_eq!(
        label_of(cx, "session-menu-pin").as_deref(),
        Some("Pin to top")
    );
    assert!(
        painted(&frame(cx))
            .iter()
            .any(|text| text == "Keep this session above unpinned ones")
    );
    test_support::keys(cx, "shift-tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-seen"),
        "the pin item comes right after Mark unread"
    );
    test_support::keys(cx, "tab enter");
    test_support::settle(cx);
    assert!(!any_layer(cx), "the menu closed");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-c"));
    assert_eq!(pin_requests(&mut fake), [pin_put("c", true)]);

    away(cx);
    push(
        &mut fake,
        &["a", "b", "c"],
        &[(session("c").pinned(1), 0)],
        cx,
    );
    assert_eq!(row_order(&frame(cx)), ["c", "a", "b"]);
    assert_eq!(label_of(cx, "session-row-c-pin").as_deref(), Some("Pinned"));
    assert_eq!(label_of(cx, "session-row-c").as_deref(), Some("Pinned c"));
    assert!(!drawn(cx, "session-row-a-pin"));
    assert!(separator_between(cx, "session-row-c", "session-row-a"));

    test_support::click(cx, "session-menu-c");
    assert_eq!(label_of(cx, "session-menu-pin").as_deref(), Some("Unpin"));
    assert!(
        painted(&frame(cx))
            .iter()
            .any(|text| text == "Sort this session with the others again")
    );
    assert!(!drawn(cx, "session-menu-move-up") && !drawn(cx, "session-menu-move-down"));
    test_support::click(cx, "session-menu-pin");
    test_support::settle(cx);
    assert_eq!(pin_requests(&mut fake), [pin_put("c", false)]);

    away(cx);
    push(&mut fake, &["a", "b", "c"], &[(session("c"), 0)], cx);
    assert_eq!(row_order(&frame(cx)), ["a", "b", "c"]);
    assert!(!drawn(cx, "session-row-c-pin"));
    assert!(!drawn(cx, "session-pin-separator"));
}

// ---- Move up and Move down -----------------------------------------------------------------------------------------

/// SPEC.md: Move up and Move down "rearrange eligible pins relative to the displayed neighbor; an unavailable direction
/// is hidden, and unpinned rows offer neither action. Accepting a move closes the menu and keeps keyboard focus on the
/// moved row's menu button", and a manual move "takes effect without waiting for the hold to end". The old `pins.spec.ts`
/// case "menu moves share manual pin order, retain focus and ignore sort choices". Specified: with b, a, c pinned at 1,
/// 2, 3 and u unpinned, all served u first, the list draws b, a, c, u; c's menu has no Move down; Enter on its Move up
/// with the pointer over the rows sends `{"neighbor": "a", "placement": "before"}`, closes the menu and leaves focus on
/// c's button; the helm's new positions show as b, c, a, u with the pointer still there, focus staying on c's button;
/// b's menu has no Move up, and its Move down sends `{"neighbor": "c", "placement": "after"}` and ends as c, b, a, u with
/// focus on b's button; u's menu offers neither.
#[gpui::test]
fn moves_name_the_displayed_neighbor_take_effect_under_the_hold_and_keep_focus(
    cx: &mut TestAppContext,
) {
    let rows = [
        (session("u"), 0),
        (session("a").pinned(2), 0),
        (session("b").pinned(1), 0),
        (session("c").pinned(3), 0),
    ];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    pin_requests(&mut fake);
    let order = ["u", "a", "b", "c"];
    assert_eq!(row_order(&frame(cx)), ["b", "a", "c", "u"]);

    test_support::click(cx, "session-menu-c");
    assert!(!drawn(cx, "session-menu-move-down"), "c is the last pin");
    test_support::keys(cx, "escape");
    focus_item(cx, "session-menu-c", "session-menu-move-up");
    assert!(
        painted(&frame(cx))
            .iter()
            .any(|text| text == "Place before the pinned session above")
    );
    over(cx, "session-row-a");
    test_support::keys(cx, "enter");
    test_support::settle(cx);
    assert!(!any_layer(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-c"));
    assert_eq!(pin_requests(&mut fake), [move_put("c", "a", "before")]);
    push(
        &mut fake,
        &order,
        &[
            (session("b").pinned(1), 0),
            (session("c").pinned(2), 0),
            (session("a").pinned(3), 0),
        ],
        cx,
    );
    assert_eq!(
        row_order(&frame(cx)),
        ["b", "c", "a", "u"],
        "the hold lets a move through"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-c"));

    test_support::click(cx, "session-menu-b");
    assert!(!drawn(cx, "session-menu-move-up"), "b is the first pin");
    test_support::keys(cx, "escape");
    focus_item(cx, "session-menu-b", "session-menu-move-down");
    test_support::keys(cx, "enter");
    test_support::settle(cx);
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-b"));
    assert_eq!(pin_requests(&mut fake), [move_put("b", "c", "after")]);
    push(
        &mut fake,
        &order,
        &[(session("c").pinned(1), 0), (session("b").pinned(2), 0)],
        cx,
    );
    assert_eq!(row_order(&frame(cx)), ["c", "b", "a", "u"]);
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-b"));

    test_support::click(cx, "session-menu-u");
    assert!(!drawn(cx, "session-menu-move-up") && !drawn(cx, "session-menu-move-down"));
}

/// SPEC.md: a pending request refuses the session's other actions, and a failure "surfaces like any other operation"
/// on its row; the old app named the two kinds "Pin: " and "Move pin: ". Specified: with a and b pinned, b's Move up
/// sent and unanswered, b's menu draws Unpin and Move up but a click on either sends nothing and leaves the menu open;
/// the helm refusing the move as stale shows "Move pin: session or neighbor is no longer pinned" under b's row; b's
/// Unpin refused shows "Pin: <words>" in its place.
#[gpui::test]
fn a_pending_pin_request_refuses_the_items_and_a_refusal_shows_on_the_row(cx: &mut TestAppContext) {
    let rows = [
        (session("a").pinned(1), 0),
        (session("b").pinned(2), 0),
        (session("u"), 0),
    ];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    pin_requests(&mut fake);
    test_support::click(cx, "session-menu-b");
    test_support::click(cx, "session-menu-move-up");
    test_support::settle(cx);
    let [pending] = &fake.commands()[..] else {
        panic!("one move was sent");
    };
    assert!(pending.path.ends_with("/b/pin-order"));
    let pending = pending.id;

    test_support::click(cx, "session-menu-b");
    for item in ["session-menu-pin", "session-menu-move-up"] {
        test_support::click(cx, item);
        assert!(
            drawn(cx, "session-menu"),
            "{item} is unavailable: the menu stays open"
        );
    }
    assert!(fake.commands().is_empty(), "nothing more was sent");
    test_support::keys(cx, "escape");

    fake.reply(
        pending,
        409,
        Some(json!("session or neighbor is no longer pinned")),
    );
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-outcome-b").as_deref(),
        Some("Move pin: session or neighbor is no longer pinned")
    );

    test_support::click(cx, "session-menu-b");
    test_support::click(cx, "session-menu-pin");
    test_support::settle(cx);
    let [unpin] = &fake.commands()[..] else {
        panic!("one unpin was sent");
    };
    fake.reply(unpin.id, 500, Some(json!("pin-failure-sentinel")));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-outcome-b").as_deref(),
        Some("Pin: pin-failure-sentinel")
    );
}

// ---- The order hold ------------------------------------------------------------------------------------------------

/// SPEC.md: "A client holds back reordering while the pointer is over the list … During a hold, rows keep updating in
/// place; a session that disappears is removed at once … and a new session appears at the bottom of the list … until
/// the hold ends", and the order shows "as soon as the pointer leaves the list". The old `sort.spec.ts` case "the order
/// holds under the pointer and catches up when it leaves". Specified: with a, b, c and the pointer over b, a push that
/// orders them c, b, a and renames b keeps a, b, c with b's new title; a new session d served first appears last; a
/// second new session e, also served first, goes below d; a deleted a goes at once; with the pointer off the rows the
/// helm's order e, d, c, b shows.
#[gpui::test]
fn the_pointer_holds_the_order_and_leaving_releases_it(cx: &mut TestAppContext) {
    let rows = [(session("a"), 0), (session("b"), 0), (session("c"), 0)];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    over(cx, "session-row-b");

    push(
        &mut fake,
        &["c", "b", "a"],
        &[(session("b").title("renamed"), 0)],
        cx,
    );
    assert_eq!(row_order(&frame(cx)), ["a", "b", "c"]);
    assert_eq!(label_of(cx, "session-row-b").as_deref(), Some("renamed"));
    push(&mut fake, &["d", "c", "b", "a"], &[(session("d"), 0)], cx);
    assert_eq!(row_order(&frame(cx)), ["a", "b", "c", "d"]);
    push(
        &mut fake,
        &["e", "d", "c", "b", "a"],
        &[(session("e"), 0)],
        cx,
    );
    assert_eq!(row_order(&frame(cx)), ["a", "b", "c", "d", "e"]);
    fake.push(change(
        listing_meta(&["e", "d", "c", "b"], 4, false),
        &[],
        &["a"],
    ));
    test_support::settle(cx);
    assert_eq!(
        row_order(&frame(cx)),
        ["b", "c", "d", "e"],
        "a deleted row goes at once"
    );

    away(cx);
    assert_eq!(row_order(&frame(cx)), ["e", "d", "c", "b"]);
}

/// SPEC.md: the order shows again once the pointer "has been completely still over it for 5 seconds", and "pointer
/// movement over the list starts the 5 seconds of stillness over". The old `sort.spec.ts` case "a still pointer releases
/// the held order after five seconds". Specified: with the pointer over b and the order pushed as c, b, a, the rows stay
/// a, b, c 4 seconds on; a move over the rows then, and 4 seconds more, still holds them; 1 second after that (5 since
/// the move) the helm's order shows.
#[gpui::test]
fn five_seconds_of_stillness_release_the_hold_and_movement_restarts_them(cx: &mut TestAppContext) {
    let rows = [(session("a"), 0), (session("b"), 0), (session("c"), 0)];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    over(cx, "session-row-b");
    reorder(&mut fake, &["c", "b", "a"], cx);
    assert_eq!(row_order(&frame(cx)), ["a", "b", "c"], "the premise: held");

    let second = Duration::from_secs(1);
    test_support::advance(cx, STILLNESS - second);
    assert_eq!(row_order(&frame(cx)), ["a", "b", "c"]);
    over(cx, "session-row-a");
    test_support::advance(cx, STILLNESS - second);
    assert_eq!(
        row_order(&frame(cx)),
        ["a", "b", "c"],
        "the move started the 5 seconds over"
    );
    test_support::advance(cx, second);
    assert_eq!(row_order(&frame(cx)), ["c", "b", "a"]);
}

/// SPEC.md: "While a session row's menu is open, the hold is kept until the menu closes, even if the pointer leaves the
/// list or stays still, because releasing it would move the very row the menu belongs to." The old `sort.spec.ts` case
/// "an open menu defers the release without losing it". Specified: with a's menu open from a click and the order pushed
/// as c, b, a, the rows stay a, b, c through more than 5 seconds of stillness and the pointer leaving, with the menu
/// still open; Escape closes it, and with the pointer off the rows the helm's order shows.
#[gpui::test]
fn an_open_row_menu_keeps_the_hold(cx: &mut TestAppContext) {
    let rows = [(session("a"), 0), (session("b"), 0), (session("c"), 0)];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    over(cx, "session-row-a");
    test_support::click(cx, "session-menu-a");
    assert!(drawn(cx, "session-menu"), "the premise: a's menu is open");
    reorder(&mut fake, &["c", "b", "a"], cx);
    test_support::advance(cx, STILLNESS + Duration::from_secs(1));
    away(cx);
    assert_eq!(row_order(&frame(cx)), ["a", "b", "c"]);
    assert!(drawn(cx, "session-menu"), "the menu is still open");

    test_support::keys(cx, "escape");
    test_support::settle(cx);
    assert_eq!(row_order(&frame(cx)), ["c", "b", "a"]);
}

/// SPEC.md: the order shows again once the pointer "has been completely still over it for 5 seconds (a scroll counts
/// as movement)". A wheel scroll moves the rows under a still pointer, so it must restart the 5 seconds like a move.
/// Specified: with the pointer over b and the order pushed as c, b, a, a wheel scroll 4 seconds on, at the same point
/// with no pointer move, keeps the rows a, b, c 4 seconds after it; 1 second more (5 since the scroll) releases them.
#[gpui::test]
fn a_scroll_over_the_rows_counts_as_movement(cx: &mut TestAppContext) {
    let rows = [(session("a"), 0), (session("b"), 0), (session("c"), 0)];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    over(cx, "session-row-b");
    let at = test_support::center(cx, "session-row-b");
    reorder(&mut fake, &["c", "b", "a"], cx);
    assert_eq!(row_order(&frame(cx)), ["a", "b", "c"], "the premise: held");

    let second = Duration::from_secs(1);
    test_support::advance(cx, STILLNESS - second);
    cx.simulate_event(ScrollEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(0.), px(-20.))),
        ..Default::default()
    });
    test_support::settle(cx);
    test_support::advance(cx, STILLNESS - second);
    assert_eq!(
        row_order(&frame(cx)),
        ["a", "b", "c"],
        "the scroll started the 5 seconds over"
    );
    test_support::advance(cx, second);
    assert_eq!(row_order(&frame(cx)), ["c", "b", "a"]);
}

/// A row's menu opens beside its button, past the sidebar's edge, but in a window too narrow for that the layer
/// manager's clamp pushes it back over part of the rows, so a pointer resting on the menu reads as off the rows; when
/// the menu closes under it, releasing the hold would reorder the rows under a pointer that never moved, which is what
/// the hold exists to prevent. GPUI keeps the pointer's last position after it leaves the window, so the close also
/// asks whether the pointer is in the window; GPUI's test window never says so on Linux, so the test hands the list
/// each answer. Specified: in a window 560 pixels wide, with the pointer in the window, a's menu opened by a click and
/// the order pushed as c, b, a, the pointer moved onto the menu where it lies over the rows, and the menu closed with
/// Escape, the rows stay a, b, c, and once the pointer leaves the rows the helm's order shows; the same close with the
/// pointer reported out of the window (its last position still on the menu) releases the hold at once.
#[gpui::test]
fn a_menu_closing_over_the_rows_keeps_the_hold(cx: &mut TestAppContext) {
    let rows = [(session("a"), 0), (session("b"), 0), (session("c"), 0)];
    let (mut fake, _connection) = test_support::started(cx);
    fake.connect(snapshot(&rows, false));
    cx.run_until_parked();
    let (main, cx) = test_support::open_view(cx, MainView::new);
    // Narrower than the sidebar and a menu beside it, so the menu is clamped back over the rows.
    cx.simulate_resize(gpui::size(px(560.), px(700.)));
    test_support::settle(cx);
    let list = cx.update(|_, cx| main.read(cx).session_list().clone());
    let mut close_on_menu =
        |in_window: fn(&gpui::Window) -> bool, order: [&str; 3], cx: &mut VisualTestContext| {
            cx.update(|_, cx| list.update(cx, |list, _| list.pointer_in_window_reads(in_window)));
            over(cx, "session-row-a");
            test_support::click(cx, "session-menu-a");
            reorder(&mut fake, &order, cx);
            let (menu, rows) = (
                test_support::bounds(cx, "session-menu"),
                test_support::bounds(cx, "session-rows"),
            );
            // The menu's height can move its top above the list. Pick a
            // point in the intersection so the pointer still holds the rows.
            let on_menu = Point::new(
                menu.left().max(rows.left()) + px(4.),
                menu.top().max(rows.top()) + px(4.),
            );
            assert!(
                menu.contains(&on_menu) && rows.contains(&on_menu),
                "the premise: the menu covers the rows at {on_menu:?}: {menu:?} over {rows:?}"
            );
            cx.simulate_mouse_move(on_menu, None, Modifiers::none());
            test_support::settle(cx);
            test_support::keys(cx, "escape");
            test_support::settle(cx);
            assert!(!any_layer(cx), "the menu closed");
        };

    close_on_menu(|_| true, ["c", "b", "a"], cx);
    assert_eq!(
        row_order(&frame(cx)),
        ["a", "b", "c"],
        "held under the pointer"
    );
    away(cx);
    assert_eq!(row_order(&frame(cx)), ["c", "b", "a"]);

    close_on_menu(|_| false, ["a", "b", "c"], cx);
    assert_eq!(
        row_order(&frame(cx)),
        ["a", "b", "c"],
        "the pointer outside the window releases the hold"
    );
}

/// SPEC.md: "Other pin and unpin changes update in place during a hold and move to their grouped positions when it
/// ends. Newly pinned rows offer no Move actions until then, including a new pin contiguous with the leading pinned run;
/// original pins cannot use that new pin as a Move neighbor during the hold." The old `pins.spec.ts` case "new pins remain
/// held across replies until the original hold ends". The boundary is the contiguous one: a pin held right under the
/// leading run is pinned and adjacent, so only the hold's record of which pins it began with keeps it out of the Move
/// neighbors (a pin held below an unpinned row would be out of the run anyway). Specified: with b and a pinned and c,
/// d unpinned, and the pointer over a, c pinned by the helm at the top stays third, right under a, with its glyph; c's
/// menu offers no Move items and a's offers Move up but no Move down, also after a later push (a rename); the
/// separator follows the displayed pinned run, now ending at c; once the pointer leaves, the order is c, b, a, d and c's
/// menu offers Move down and no Move up.
#[gpui::test]
fn a_pin_made_during_the_hold_waits_in_place_with_no_moves(cx: &mut TestAppContext) {
    let rows = [
        (session("b").pinned(1), 0),
        (session("a").pinned(2), 0),
        (session("c"), 0),
        (session("d"), 0),
    ];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    let order = ["b", "a", "c", "d"];
    over(cx, "session-row-a");

    push(&mut fake, &order, &[(session("c").pinned(0), 0)], cx);
    assert_eq!(row_order(&frame(cx)), ["b", "a", "c", "d"], "held");
    assert!(drawn(cx, "session-row-c-pin"), "the premise: c is pinned");
    assert!(separator_between(cx, "session-row-c", "session-row-d"));
    for _ in 0..2 {
        test_support::click(cx, "session-menu-c");
        assert!(!drawn(cx, "session-menu-move-up") && !drawn(cx, "session-menu-move-down"));
        test_support::keys(cx, "escape");
        test_support::click(cx, "session-menu-a");
        assert!(drawn(cx, "session-menu-move-up"));
        assert!(
            !drawn(cx, "session-menu-move-down"),
            "the new pin right under a is no neighbor"
        );
        test_support::keys(cx, "escape");
        push(&mut fake, &order, &[(session("d").title("renamed"), 0)], cx);
        assert_eq!(row_order(&frame(cx)), ["b", "a", "c", "d"]);
    }

    away(cx);
    assert_eq!(row_order(&frame(cx)), ["c", "b", "a", "d"]);
    assert!(separator_between(cx, "session-row-a", "session-row-d"));
    test_support::click(cx, "session-menu-c");
    assert!(drawn(cx, "session-menu-move-down"));
    assert!(!drawn(cx, "session-menu-move-up"));
}

/// The pinned partition is drawn whatever the helm's served order, from the store, so a listing narrowed by a filter
/// keeps its pins first and in their shared order (SPEC.md: "host selection and other filters still apply without
/// changing the stored order"). Specified: served u, b (2), a (1) with the pointer off the rows, the list draws a, b,
/// then u; served as only u and b (a no longer served, as a filter would leave it), b, u.
#[gpui::test]
fn a_narrowed_listing_keeps_its_pins_first_in_their_order(cx: &mut TestAppContext) {
    let rows = [
        (session("u"), 0),
        (session("b").pinned(2), 0),
        (session("a").pinned(1), 0),
    ];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    away(cx);
    assert_eq!(row_order(&frame(cx)), ["a", "b", "u"]);
    fake.push(change(listing_meta(&["u", "b"], 3, false), &[], &["a"]));
    test_support::settle(cx);
    assert_eq!(row_order(&frame(cx)), ["b", "u"]);
}

/// The fixture default matters to every other test of the list and to the screenshot goldens: the helm's own list
/// fixture row is pinned, which would group every row first and draw a glyph on each. Specified: the area's `Fixture`
/// is unpinned with no position unless a test names one, and then carries it.
#[test]
fn fixtures_are_unpinned_unless_they_name_a_position() {
    let row = session("a").row();
    assert!(!row.pinned && row.pin_position.is_none());
    let row = session("a").pinned(4).row();
    assert!(row.pinned && row.pin_position == Some(4));
}

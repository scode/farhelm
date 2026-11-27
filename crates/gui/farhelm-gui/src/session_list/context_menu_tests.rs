//! Tests of the session list's context menus: a right press on a session row or a group header, or Shift+F10 on a
//! focused one, opens its actions menu (at the pointer, or beside the row for the key), one press moves the menu from
//! one row to another, and with two or more rows marked a right press on a marked row opens the marked menu with the
//! bar's lifecycle actions too (SPEC.md, "Session list").
//!
//! The menus themselves (what each command does) are tested from their ⋯ buttons in `menu_tests`, `marking_tests` and
//! `bulk_tests`; these pin only the gestures, which menu each opens, where it opens and what it leaves alone. They run
//! the real main view over the helm client's fake, as those do, and read the menus from GPUI's driver record by their
//! ids: `session-menu` and `session-menu-items` for a row's, `session-group-menu-items` for a group's, and
//! `session-marked-menu` with its `session-marked-menu-*` items for the marked one. A right press is what GPUI delivers
//! for a macOS Ctrl+click too (its platform layer turns one into the other before any element sees it), so the right
//! press stands for both.

use farhelm_helm_client::fake::FakeHelm;
use gpui::{Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px};

use super::groups_tests::{group, grouped_snapshot};
use super::marking::{
    COUNT_ID, MENU_DELETE_ID, MENU_ID, MENU_START_ID, MENU_STOP_ID, MENU_TOGGLE_ID,
};
use super::marking_tests::{click_with, count, ctrl, open_list, requests};
use super::menu_tests::{any_layer, drawn, ended, label_of, push, snapshot};
use super::tests::{open_main, selected, session};
use crate::focus;
use crate::test_support::{self, focused_id, keys};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// A right press and release at `at`, as a secondary click delivers them, then a settle.
fn right_press(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    test_support::settle(cx);
}

/// A point inside row `key`'s open control, well left of its marks and its ⋯ button: a right press on the row's body.
fn on_row(cx: &mut VisualTestContext, key: &str) -> Point<Pixels> {
    let id = format!("session-row-{key}");
    let nodes = test_support::frame(cx);
    let row = super::row_tests::node(&nodes, &id)
        .unwrap_or_else(|| panic!("{id} is drawn"))
        .bounds;
    point(row.left() + px(40.), row.center().y)
}

/// Whether the popover drawn under `id` has its top-left corner at `at`, to the pixel the layer manager snaps it to.
fn opens_at(cx: &mut VisualTestContext, id: &'static str, at: Point<Pixels>) -> bool {
    let origin = test_support::bounds(cx, id).origin;
    (origin.x - at.x).abs() < px(1.) && (origin.y - at.y).abs() < px(1.)
}

/// The accessible name of the open row menu's commands, which names its session.
fn row_menu(cx: &mut VisualTestContext) -> Option<String> {
    label_of(cx, "session-menu-items")
}

/// Mark each of `rows` (row selectors) with the Ctrl-click toggle.
fn mark(cx: &mut VisualTestContext, rows: &[&'static str]) {
    for row in rows {
        click_with(cx, row, ctrl());
    }
}

// ---- One row -------------------------------------------------------------------------------------------------------

/// SPEC.md: "A right press anywhere on a session row ... opens that row's actions menu at the pointer, kept inside the
/// window, with the same commands as its ⋯ button". A desktop list opens its context menu where the user pressed, and
/// the menu must belong to the row pressed, without opening that session or changing the marks, or a look at a row's
/// actions would move the user away from what they were working on. Specified: with a open and b listed, a right press
/// on b's body opens b's menu ("Session actions for b") with its top-left corner at the press (to the pixel the layer
/// manager snaps to) and focus on Rename, sends nothing and leaves a open; Escape closes it with focus back where it was
/// before the press, not on b's ⋯ button (SPEC.md, "Keyboard focus": a right press moves no focus, and closing returns
/// focus where it was); a right press on b's status dot opens b's menu too.
#[gpui::test]
fn a_right_press_on_a_row_opens_its_menu_at_the_pointer(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) =
        open_list(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    requests(&mut fake);
    assert_eq!(selected(cx).as_deref(), Some("a"), "premise: a is open");
    let before = focused_id(cx);
    assert_ne!(
        before.as_deref(),
        Some("session-menu-b"),
        "premise: focus starts somewhere other than b's ⋯ button"
    );

    let at = on_row(cx, "b");
    right_press(cx, at);
    assert_eq!(row_menu(cx).as_deref(), Some("Session actions for b"));
    assert!(
        opens_at(cx, "session-menu", at),
        "the menu's corner is at the press"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));
    assert_eq!(requests(&mut fake), [], "opening the menu sent nothing");
    assert_eq!(selected(cx).as_deref(), Some("a"), "b was not opened");

    keys(cx, "escape");
    assert!(!any_layer(cx), "Escape closed the menu");
    assert_eq!(
        focused_id(cx),
        before,
        "focus is back where it was before the press, not on the ⋯ button"
    );

    let dot = test_support::center(cx, "session-seen-b");
    right_press(cx, dot);
    assert_eq!(
        row_menu(cx).as_deref(),
        Some("Session actions for b"),
        "a press on the row's status dot opens the row's menu"
    );
}

/// SPEC.md: "A right press while another row's menu or the marked menu is open closes that menu and opens the new one in
/// the same press". The layer manager leaves a press on a popover's own anchor (the row's ⋯ button) to the anchor, so
/// without the list closing the open menu itself a second right press could stack a second menu or need a second
/// press. A menu opened at the pointer covers the rows below the press, so the second press is on a row above it.
/// Specified: with b's menu open from a right press, a right press on a leaves exactly one row menu, a's, at that press
/// with focus on Rename; a right press on a's ⋯ button then leaves exactly one, a's again, now at that press, with focus
/// on Rename.
#[gpui::test]
fn one_right_press_moves_the_menu_to_another_row(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) =
        open_list(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    requests(&mut fake);
    let at = on_row(cx, "b");
    right_press(cx, at);
    assert_eq!(row_menu(cx).as_deref(), Some("Session actions for b"));

    let at = on_row(cx, "a");
    right_press(cx, at);
    let menus = |cx: &mut VisualTestContext| {
        test_support::frame(cx)
            .into_iter()
            .filter(|node| node.element_id.as_deref() == Some("session-menu"))
            .count()
    };
    assert_eq!(menus(cx), 1, "one menu");
    assert_eq!(row_menu(cx).as_deref(), Some("Session actions for a"));
    assert!(
        opens_at(cx, "session-menu", at),
        "a's menu is at the second press"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));

    let button = test_support::center(cx, "session-menu-a");
    right_press(cx, button);
    assert_eq!(menus(cx), 1, "one menu after a press on its own anchor");
    assert_eq!(row_menu(cx).as_deref(), Some("Session actions for a"));
    assert!(
        opens_at(cx, "session-menu", button),
        "the menu moved to the press on the button: the open one closed first"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));
}

/// SPEC.md: "Shift+F10 on a focused row or group header, or on a control inside it, opens its menu beside the row,
/// where ⋯ opens it". It is the platforms' keyboard chord for a context menu, so a keyboard user reaches the same
/// commands as a right press; only that chord, since F10 alone is some other shortcut's. Specified: with focus on b's ⋯
/// button, a control inside b's row (its menu opened and closed with Escape, which hands focus back to the button), F10
/// alone opens nothing; Shift+F10 opens b's menu with focus on Rename, its left edge 10 pixels past b's ⋯ button (the
/// beside placement's gap), and sends nothing.
#[gpui::test]
fn shift_f10_on_a_focused_row_opens_its_menu_beside_it(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) =
        open_list(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    requests(&mut fake);
    test_support::click(cx, "session-menu-b");
    keys(cx, "escape");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-b"),
        "premise: focus is on b's ⋯ button, inside b's row"
    );
    keys(cx, "f10");
    assert!(!any_layer(cx), "F10 alone opens nothing");
    keys(cx, "shift-f10");
    assert_eq!(row_menu(cx).as_deref(), Some("Session actions for b"));
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));
    let button = test_support::bounds(cx, "session-menu-b");
    assert_eq!(
        test_support::bounds(cx, "session-menu").left(),
        button.right() + px(10.)
    );
    assert_eq!(requests(&mut fake), [], "opening the menu sent nothing");
}

/// SPEC.md: "A right press on a group header opens the group's menu the same way". A group's ⋯ menu is the only other
/// row menu in the list, and a right press on its header must not open one of its members' menus instead. Specified:
/// with sessions m (in group 10, "Project") and loose, a right press on the group's header (on its name, the expand
/// toggle) opens the group's menu ("Group actions") at the press and no session's menu.
#[gpui::test]
fn a_right_press_on_a_group_header_opens_the_groups_menu(cx: &mut TestAppContext) {
    let state = grouped_snapshot(
        &[session("m").grouped(10), session("loose")],
        vec![group(10, "Project", 1, false)],
    );
    let (_fake, cx) = open_main(cx, state);
    let at = test_support::center(cx, "session-group-10-toggle");
    right_press(cx, at);
    assert_eq!(
        label_of(cx, "session-group-menu-items").as_deref(),
        Some("Group actions")
    );
    assert!(opens_at(cx, "session-menu", at));
    assert_eq!(row_menu(cx), None, "no session's menu opened");
}

// ---- Marked rows ---------------------------------------------------------------------------------------------------

/// SPEC.md: "With two or more marked rows displayed, a right press on a marked row opens a combined menu instead: the
/// bar's applicable lifecycle actions (Restart/Replace, Stop, Delete), then a divider and the metadata actions, each
/// acting as the bar's control does". A right press on one of several marked rows is a request about the marks, and
/// the menu must offer what the bar offers, so the user need not travel to the bar. Specified: with a, b and c ended
/// and Skip session confirmations on, a and b marked, a right press on b opens the marked menu ("2 marked") at the press
/// with Restart/Replace (focused), Stop and Delete, and the metadata actions (Pin among them), and no row menu; the
/// menu's Delete closes it and, asking nothing, sends a's Delete (with the guard an ended session's Delete carries).
#[gpui::test]
fn a_right_press_on_a_marked_row_opens_the_marked_menu_with_lifecycle_actions(
    cx: &mut TestAppContext,
) {
    let (mut fake, _list, cx) = open_list(
        cx,
        snapshot(&[(ended("a"), 0), (ended("b"), 0), (ended("c"), 0)], true),
    );
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    requests(&mut fake);
    assert_eq!(count(cx).as_deref(), Some("2 marked"), "premise: two marks");

    let at = on_row(cx, "b");
    right_press(cx, at);
    assert!(drawn(cx, MENU_ID), "the marked menu opened");
    assert_eq!(row_menu(cx), None, "no row menu opened");
    assert!(opens_at(cx, MENU_ID, at));
    for item in [
        MENU_START_ID,
        MENU_STOP_ID,
        MENU_DELETE_ID,
        "session-marked-menu-pin",
    ] {
        assert!(drawn(cx, item), "{item}");
    }
    assert_eq!(focused_id(cx).as_deref(), Some(MENU_START_ID));
    assert_eq!(requests(&mut fake), [], "opening the menu sent nothing");

    test_support::click(cx, MENU_DELETE_ID);
    assert!(!drawn(cx, MENU_ID), "choosing closed the menu");
    let sent: Vec<String> = fake
        .commands()
        .into_iter()
        .map(|command| command.path)
        .collect();
    assert_eq!(
        sent,
        ["/api/sessions/a?only_if_nothing_alive=true"],
        "the batch's first Delete went out, with the ended session's usual guard"
    );
}

/// SPEC.md: "A right press on an unmarked row then opens that row's own menu and leaves the marks as they are". The
/// plan's decision: the marks are the user's deliberate choice, and a press on another row asks about that row. Only
/// a marked row's press asks about the marks. Specified: with a and b marked, a right press on c opens c's own menu,
/// not the marked menu, and the bar still says "2 marked".
#[gpui::test]
fn a_right_press_on_an_unmarked_row_opens_its_own_menu_and_keeps_the_marks(
    cx: &mut TestAppContext,
) {
    let (mut fake, _list, cx) = open_list(
        cx,
        snapshot(
            &[(session("a"), 0), (session("b"), 0), (session("c"), 0)],
            false,
        ),
    );
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    requests(&mut fake);
    let at = on_row(cx, "c");
    right_press(cx, at);
    assert_eq!(row_menu(cx).as_deref(), Some("Session actions for c"));
    assert!(!drawn(cx, MENU_ID), "not the marked menu");
    assert_eq!(count(cx).as_deref(), Some("2 marked"), "the marks stay");
}

/// SPEC.md: "with exactly one row marked, a right press on it opens its own menu". One mark is no batch, and the row's
/// own menu offers more than the marked one. Specified: with only b marked, a right press on b opens b's own menu, not
/// the marked menu, and the bar still says "1 marked".
#[gpui::test]
fn with_one_mark_a_right_press_on_it_opens_its_own_menu(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) =
        open_list(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    requests(&mut fake);
    mark(cx, &["session-row-b"]);
    requests(&mut fake);
    assert_eq!(count(cx).as_deref(), Some("1 marked"), "premise: one mark");
    let at = on_row(cx, "b");
    right_press(cx, at);
    assert_eq!(row_menu(cx).as_deref(), Some("Session actions for b"));
    assert!(!drawn(cx, MENU_ID));
    assert_eq!(count(cx).as_deref(), Some("1 marked"), "the mark stays");
}

/// The bar's ⋯ keeps its metadata-only menu: the lifecycle actions are already the bar's own buttons beside it, and
/// repeating them in its menu would show each twice. Specified: with a and b ended and marked, the bar's ⋯ opens the
/// marked menu without Restart/Replace, Stop or Delete, with Pin.
#[gpui::test]
fn the_bars_menu_keeps_only_the_metadata_actions(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(cx, snapshot(&[(ended("a"), 0), (ended("b"), 0)], false));
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    requests(&mut fake);
    test_support::click(cx, MENU_TOGGLE_ID);
    assert!(drawn(cx, MENU_ID));
    for item in [MENU_START_ID, MENU_STOP_ID, MENU_DELETE_ID] {
        assert!(!drawn(cx, item), "{item}");
    }
    assert!(drawn(cx, "session-marked-menu-pin"));
}

/// A marked menu a right press or Shift+F10 opened on a row is that row's open menu, so the row's ⋯ button closes it
/// rather than stacking the row's own menu on it. The layer manager leaves a press on a popover's own anchor (the ⋯
/// button) to the anchor, so without that record a click there opened a second menu over the first, and Escape then
/// left the marked menu open with focus outside it. Specified: with a and b marked and focus on b's ⋯ button (its menu
/// opened and closed with Escape), Shift+F10 opens the marked menu; a click on b's ⋯ then closes it and opens no menu
/// at all.
#[gpui::test]
fn the_rows_button_closes_a_marked_menu_opened_from_its_row(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(cx, snapshot(&[(ended("a"), 0), (ended("b"), 0)], false));
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    requests(&mut fake);
    test_support::click(cx, "session-menu-b");
    keys(cx, "escape");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-b"),
        "premise: focus is on b's ⋯ button"
    );
    keys(cx, "shift-f10");
    assert!(drawn(cx, MENU_ID), "the marked menu opened");
    test_support::click(cx, "session-menu-b");
    assert!(!drawn(cx, MENU_ID), "the click closed the marked menu");
    assert_eq!(row_menu(cx), None, "and opened no row menu");
    assert!(!any_layer(cx));
}

/// A right press's marked menu offers the lifecycle actions the bar offered when it opened, and it is a menu about two or
/// more marks, so it must close when those actions change or fewer than two marks are shown, as the bar's own menu
/// closes when its metadata actions change. Otherwise its Restart/Replace would stay on offer for a selection the bar no
/// longer offers it for, or it would act on a single mark as a batch. The metadata actions must stay the same across
/// each change, so that only this rule can close the menu. Specified: with a and b running with their terminals gone
/// (which offers Restart/Replace) and marked, the marked menu a right press on b opened offers Restart/Replace and Mark
/// unread; once the helm lists a with its terminal back, a still offering Mark unread, the menu is gone. Opened again
/// by a right press on a, with b with its terminal gone, it is gone once the helm stops listing b, which leaves a's
/// mark alone.
#[gpui::test]
fn a_marked_context_menu_closes_when_its_lifecycle_actions_change(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(
        cx,
        snapshot(
            &[
                (session("a").terminal_missing(), 0),
                (session("b").terminal_missing(), 0),
            ],
            false,
        ),
    );
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    requests(&mut fake);
    let at = on_row(cx, "b");
    right_press(cx, at);
    assert!(
        drawn(cx, MENU_START_ID),
        "premise: Restart/Replace is offered"
    );
    assert!(
        drawn(cx, "session-marked-menu-unread"),
        "premise: Mark unread is offered"
    );
    push(
        &mut fake,
        &["a", "b"],
        &[(session("a"), 0), (session("b").terminal_missing(), 0)],
        cx,
    );
    assert!(
        !drawn(cx, MENU_ID),
        "the menu closed with its lifecycle offer"
    );

    push(
        &mut fake,
        &["a", "b"],
        &[
            (session("a").terminal_missing(), 0),
            (session("b").terminal_missing(), 0),
        ],
        cx,
    );
    let at = on_row(cx, "a");
    right_press(cx, at);
    assert!(drawn(cx, MENU_ID), "premise: the marked menu is open again");
    push(
        &mut fake,
        &["a"],
        &[(session("a").terminal_missing(), 0)],
        cx,
    );
    assert!(
        !drawn(cx, MENU_ID),
        "the menu closed with fewer than two marks"
    );
}

/// A right press on a marked row's ⋯ while that row's own question is open must open nothing: the button is inert then,
/// as SPEC.md has a row's lifecycle confirmation refuse another lifecycle action. The layer manager leaves a press on
/// the question's anchor (the ⋯ button) to the anchor, so without the list's own check the marked menu opened over the
/// question and overwrote the row's record of it, which ended the order hold and let the button reopen menus over its
/// own question. Specified: with a and b marked, b's Delete question open from its ⋯ menu, a right press on b's ⋯ opens
/// no marked menu and leaves the question open.
#[gpui::test]
fn a_right_press_on_a_marked_rows_button_while_it_asks_opens_nothing(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) =
        open_list(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    requests(&mut fake);
    mark(cx, &["session-row-a", "session-row-b"]);
    requests(&mut fake);
    test_support::click(cx, "session-menu-b");
    test_support::click(cx, "session-menu-delete");
    assert!(
        drawn(cx, "prompt-confirm"),
        "premise: b's Delete question is open"
    );
    let button = test_support::center(cx, "session-menu-b");
    right_press(cx, button);
    assert!(!drawn(cx, MENU_ID), "no marked menu opened");
    assert!(drawn(cx, "prompt-confirm"), "the question is still open");
}

// ---- Focus after the marked menu's dialogs ------------------------------------------------------------------------
//
// SPEC.md, "Keyboard focus": closing something returns focus to where it was before it opened, and a right press moves
// no focus. A dialog a right press's marked menu opened (the bulk Stop or Delete question, Move to group's New group…)
// is part of that gesture, so backing out of it must put focus back where the user was before the right press, usually
// the terminal. Before the fix it went to the marked bar's ⋯, a control the user never touched, and the next keystroke
// meant for the terminal landed on the bar. The same dialogs opened from the bar keep returning to the bar's ⋯. Every
// test here runs under both platforms' click rules: on macOS a click focuses no control, so the menu items, the bar's
// buttons and its ⋯ never hold focus themselves, and only the control a dialog names (or the restored opener) can put
// focus anywhere.

/// Sessions a (running, one terminal tab, so its Stop asks) and b (running, so its Delete asks), both marked, with
/// Skip session confirmations off, under the click rule `click_focuses`, and focus on a's ⋯ button (its menu opened and
/// closed with Escape, which hands focus to the button under either rule); returns the fake, the window, and that
/// button's id.
///
/// Focus starts on a's ⋯ rather than wherever the window rested because the window rests on the open session's
/// terminal, which is also where focus recovers when a control it should return to has gone: a test starting there
/// could not tell "back where it was" from "recovered home", and would pass if the menu sent focus home.
fn two_marked_asking(
    cx: &mut TestAppContext,
    click_focuses: bool,
) -> (FakeHelm, &mut VisualTestContext, Option<String>) {
    let (mut fake, _list, cx) =
        open_list(cx, snapshot(&[(session("a"), 1), (session("b"), 0)], false));
    requests(&mut fake);
    let resting = focused_id(cx);
    cx.update(|_, cx| focus::set_click_focuses_controls(click_focuses, cx));
    mark(cx, &["session-row-a", "session-row-b"]);
    requests(&mut fake);
    assert_eq!(count(cx).as_deref(), Some("2 marked"), "premise: two marks");
    test_support::click(cx, "session-menu-a");
    keys(cx, "escape");
    let before = focused_id(cx);
    assert_eq!(
        before.as_deref(),
        Some("session-menu-a"),
        "premise: focus is on a's ⋯ button"
    );
    assert_ne!(
        before, resting,
        "premise: that is not where the window rests"
    );
    (fake, cx, before)
}

/// The bulk Stop and Delete questions a right press's marked menu asks return focus to where it was before the right
/// press when the user backs out (SPEC.md, "Keyboard focus"), and Confirm still sends it to "N marked" (SPEC.md:
/// "After confirmation closes the question, keyboard focus returns to the marked count"). Specified, under each click
/// rule: with a and b marked and focus on a's ⋯ button, for Stop and then Delete, a right press on b, the marked menu's
/// item, and Cancel (then, for the same item again, Escape) each leave no question open, send nothing, keep both marks
/// and put focus back on a's ⋯ button, where it was before the press, not the bar's ⋯; a right press, Stop and Confirm
/// then focus the marked count.
fn right_press_bulk_questions_return_focus_where_it_was(
    cx: &mut TestAppContext,
    click_focuses: bool,
) {
    let (mut fake, cx, before) = two_marked_asking(cx, click_focuses);
    for item in [MENU_STOP_ID, MENU_DELETE_ID] {
        for dismissal in ["cancel", "escape"] {
            let at = on_row(cx, "b");
            right_press(cx, at);
            test_support::click(cx, item);
            test_support::settle(cx);
            assert!(drawn(cx, "prompt"), "{item}: premise: the question asks");
            match dismissal {
                "cancel" => test_support::click(cx, "prompt-cancel"),
                _ => keys(cx, "escape"),
            }
            test_support::settle(cx);
            assert!(
                !drawn(cx, "prompt"),
                "{item} {dismissal}: the question closed"
            );
            assert_eq!(requests(&mut fake), [], "{item} {dismissal}: nothing sent");
            assert_eq!(count(cx).as_deref(), Some("2 marked"), "{item} {dismissal}");
            assert_eq!(
                focused_id(cx),
                before,
                "{item} {dismissal}: focus is back where it was before the right press"
            );
        }
    }

    let at = on_row(cx, "b");
    right_press(cx, at);
    test_support::click(cx, MENU_STOP_ID);
    test_support::settle(cx);
    test_support::click(cx, "prompt-confirm");
    test_support::settle(cx);
    assert!(!drawn(cx, "prompt"));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(COUNT_ID),
        "Confirm focuses the marked count"
    );
}

/// [`right_press_bulk_questions_return_focus_where_it_was`] where a click focuses the control it lands on (Linux).
#[gpui::test]
fn right_press_bulk_questions_return_focus_where_it_was_on_linux(cx: &mut TestAppContext) {
    right_press_bulk_questions_return_focus_where_it_was(cx, true);
}

/// [`right_press_bulk_questions_return_focus_where_it_was`] under the macOS click rule, where a click focuses nothing.
#[gpui::test]
fn right_press_bulk_questions_return_focus_where_it_was_on_macos(cx: &mut TestAppContext) {
    right_press_bulk_questions_return_focus_where_it_was(cx, false);
}

/// When what held focus before the right press goes while the bulk question is open, Cancel cannot return there and
/// must not fall back to the bar's ⋯ either: focus recovers to the open session's terminal or the window's other
/// resting place (SPEC.md, "Where focus rests"), as for any control that vanishes. Specified: with a open, a (one tab)
/// and b marked and c listed, focus on c's ⋯ button (its menu opened and closed with Escape), a right press on b and
/// the marked menu's Stop ask; once the helm stops listing c, Cancel leaves focus where the window rested before any
/// of this, not on the bar's ⋯. Under each click rule.
fn a_right_press_question_whose_opener_went_recovers_focus(
    cx: &mut TestAppContext,
    click_focuses: bool,
) {
    let (mut fake, _list, cx) = open_list(
        cx,
        snapshot(
            &[(session("a"), 1), (session("b"), 0), (session("c"), 0)],
            false,
        ),
    );
    requests(&mut fake);
    let resting = focused_id(cx);
    assert!(
        resting.is_some(),
        "premise: the window rests on some control"
    );
    cx.update(|_, cx| focus::set_click_focuses_controls(click_focuses, cx));
    mark(cx, &["session-row-a", "session-row-b"]);
    requests(&mut fake);
    assert_eq!(count(cx).as_deref(), Some("2 marked"), "premise: two marks");
    test_support::click(cx, "session-menu-c");
    keys(cx, "escape");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-c"),
        "premise: focus is on c's ⋯ button"
    );
    assert_ne!(
        resting.as_deref(),
        Some("session-menu-c"),
        "premise: the window rested elsewhere"
    );

    let at = on_row(cx, "b");
    right_press(cx, at);
    test_support::click(cx, MENU_STOP_ID);
    test_support::settle(cx);
    assert!(drawn(cx, "prompt"), "premise: the question asks");
    push(
        &mut fake,
        &["a", "b"],
        &[(session("a"), 1), (session("b"), 0)],
        cx,
    );
    assert!(!drawn(cx, "session-menu-c"), "premise: c is gone");
    assert!(drawn(cx, "prompt"), "premise: the question is still open");

    test_support::click(cx, "prompt-cancel");
    test_support::settle(cx);
    assert!(!drawn(cx, "prompt"));
    assert_eq!(
        focused_id(cx),
        resting,
        "focus recovered to where the window rests"
    );
}

/// [`a_right_press_question_whose_opener_went_recovers_focus`] where a click focuses the control it lands on (Linux).
#[gpui::test]
fn a_right_press_question_whose_opener_went_recovers_focus_on_linux(cx: &mut TestAppContext) {
    a_right_press_question_whose_opener_went_recovers_focus(cx, true);
}

/// [`a_right_press_question_whose_opener_went_recovers_focus`] under the macOS click rule, where a click focuses
/// nothing.
#[gpui::test]
fn a_right_press_question_whose_opener_went_recovers_focus_on_macos(cx: &mut TestAppContext) {
    a_right_press_question_whose_opener_went_recovers_focus(cx, false);
}

/// Move to group's New group… returns focus, when the user backs out of it, to where the marked menu that offered it
/// returns focus: where it was before the right press for a right press's menu, and the bar's ⋯ for the bar's own
/// menu (whose click, on macOS, never focused the ⋯). Specified, under each click rule: with a and b marked and focus
/// on a's ⋯ button, a right press on b, Move to group and New group… open the group name editor, and Escape closes it
/// with both marks kept and focus back on a's ⋯ button, where it was before the press; the bar's ⋯, Move to group and
/// New group… then open it again, and Escape puts focus on the bar's ⋯.
fn new_group_returns_focus_where_its_menu_does(cx: &mut TestAppContext, click_focuses: bool) {
    let (mut fake, cx, before) = two_marked_asking(cx, click_focuses);
    let at = on_row(cx, "b");
    right_press(cx, at);
    assert!(drawn(cx, MENU_ID), "premise: the marked menu opened");
    test_support::click(cx, "session-marked-menu-group");
    test_support::click(cx, "session-marked-menu-group-new");
    assert!(
        drawn(cx, "rename-field"),
        "premise: the group name editor opened"
    );
    keys(cx, "escape");
    assert!(!drawn(cx, "rename-field"));
    assert_eq!(count(cx).as_deref(), Some("2 marked"), "the marks stay");
    assert_eq!(
        focused_id(cx),
        before,
        "from a right press: back where focus was before the press"
    );

    test_support::click(cx, MENU_TOGGLE_ID);
    test_support::click(cx, "session-marked-menu-group");
    test_support::click(cx, "session-marked-menu-group-new");
    assert!(
        drawn(cx, "rename-field"),
        "premise: the editor opened from the bar"
    );
    keys(cx, "escape");
    assert!(!drawn(cx, "rename-field"));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(MENU_TOGGLE_ID),
        "from the bar's ⋯: back on the ⋯"
    );
    assert_eq!(requests(&mut fake), [], "backing out sent nothing");
}

/// [`new_group_returns_focus_where_its_menu_does`] where a click focuses the control it lands on (Linux).
#[gpui::test]
fn new_group_returns_focus_where_its_menu_does_on_linux(cx: &mut TestAppContext) {
    new_group_returns_focus_where_its_menu_does(cx, true);
}

/// [`new_group_returns_focus_where_its_menu_does`] under the macOS click rule, where a click focuses nothing.
#[gpui::test]
fn new_group_returns_focus_where_its_menu_does_on_macos(cx: &mut TestAppContext) {
    new_group_returns_focus_where_its_menu_does(cx, false);
}

/// SPEC.md: "Shift+F10 on a focused row or group header, or on a control inside it, opens its menu beside the row".
/// Specified: with focus on the group's ⋯ button (its menu opened and closed with Escape), Shift+F10 opens the group's
/// menu ("Group actions") with its left edge 10 pixels past that button.
#[gpui::test]
fn shift_f10_on_a_group_header_opens_its_menu_beside_it(cx: &mut TestAppContext) {
    let state = grouped_snapshot(
        &[session("m").grouped(10), session("loose")],
        vec![group(10, "Project", 1, false)],
    );
    let (_fake, cx) = open_main(cx, state);
    test_support::click(cx, "session-menu-group/10");
    keys(cx, "escape");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-group/10"),
        "premise: focus is on the group's ⋯ button"
    );
    keys(cx, "shift-f10");
    assert_eq!(
        label_of(cx, "session-group-menu-items").as_deref(),
        Some("Group actions")
    );
    let button = test_support::bounds(cx, "session-menu-group/10");
    assert_eq!(
        test_support::bounds(cx, "session-menu").left(),
        button.right() + px(10.)
    );
}

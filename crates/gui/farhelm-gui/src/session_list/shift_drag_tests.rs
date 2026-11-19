//! Drive range sweeps through the same pointer dispatch as the existing grip drags. The painted marks and the
//! stored selection distinguish canceled gestures from a row click that merely happened to open the same session.

use std::time::Duration;

use gpui::{
    InputEvent, KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseUpEvent, Pixels, Point,
    TestAppContext, VisualTestContext, point, px,
};

use super::groups_tests::{group, grouped_snapshot};
use super::marking_tests::{click_with, ctrl, fleet, markers, open_list, stored};
use super::tests::{change, listing_meta, row_order, selected, session};
use crate::test_support::{self, frame};

fn shift() -> Modifiers {
    Modifiers {
        shift: true,
        ..Modifiers::default()
    }
}

/// Rest the pointer before pressing. That lets the list enter its normal order hold, as it does for a user.
fn press(cx: &mut VisualTestContext, selector: &'static str, mods: Modifiers) -> Point<Pixels> {
    let at = test_support::center(cx, selector);
    cx.simulate_mouse_move(at, None, mods);
    cx.simulate_mouse_down(at, MouseButton::Left, mods);
    test_support::settle(cx);
    at
}

/// Read the current element position on each move, since the first mark replaces the list heading.
fn drag_to(cx: &mut VisualTestContext, selector: &'static str, mods: Modifiers) -> Point<Pixels> {
    let at = test_support::center(cx, selector);
    cx.simulate_mouse_move(at, Some(MouseButton::Left), mods);
    test_support::settle(cx);
    at
}

fn release(cx: &mut VisualTestContext, at: Point<Pixels>, mods: Modifiers) {
    cx.simulate_mouse_up(at, MouseButton::Left, mods);
    test_support::settle(cx);
}

/// The press is the anchor, even when a different session is open. Moving back shrinks the range and returning to
/// the source ends with that row alone. A later press outside the rows must not revive the original button click.
#[gpui::test]
fn a_sweep_shrinks_back_to_its_start_without_leaving_a_click(cx: &mut TestAppContext) {
    let (_fake, _list, cx) = open_list(cx, fleet(&["a", "b", "c", "d"]));
    assert_eq!(selected(cx).as_deref(), Some("a"));
    click_with(cx, "session-row-a", ctrl());
    assert_eq!(markers(cx), ["a"]);
    press(cx, "session-row-b", shift());
    drag_to(cx, "session-row-d", shift());
    assert_eq!(markers(cx), ["b", "c", "d"]);
    drag_to(cx, "session-row-c", shift());
    assert_eq!(markers(cx), ["b", "c"]);
    let at = drag_to(cx, "session-row-b", shift());
    assert_eq!(markers(cx), ["b"]);
    release(cx, at, Modifiers::none());
    assert_eq!(markers(cx), ["b"]);
    assert_eq!(selected(cx).as_deref(), Some("a"));

    // The original press was over b. A later click from the list's empty space to b has no row press to finish.
    let view = test_support::bounds(cx, "session-rows");
    let empty = point(view.center().x, view.bottom() - px(10.));
    assert!(empty.y > test_support::bounds(cx, "session-row-d").bottom());
    cx.simulate_mouse_move(empty, None, Modifiers::none());
    cx.simulate_mouse_down(empty, MouseButton::Left, Modifiers::none());
    release(cx, at, Modifiers::none());
    assert_eq!(markers(cx), ["b"], "there was no leftover click");
    assert_eq!(selected(cx).as_deref(), Some("a"));
}

/// GPUI can still have an unrelated plain press pending after a lost release. Ending a sweep on that old row must
/// preserve the range and the open session, even if the stale press would otherwise open the old row.
#[gpui::test]
fn a_sweep_release_cannot_finish_an_older_plain_press(cx: &mut TestAppContext) {
    let (_fake, _list, cx) = open_list(cx, fleet(&["a", "b", "c"]));
    assert_eq!(selected(cx).as_deref(), Some("a"));
    press(cx, "session-row-c", Modifiers::none());
    // The first MouseUp never reaches the window. Pressing again at b does not replace c's native pending press.
    press(cx, "session-row-b", shift());
    let at = drag_to(cx, "session-row-c", shift());
    assert_eq!(markers(cx), ["b", "c"]);
    release(cx, at, Modifiers::none());
    assert_eq!(markers(cx), ["b", "c"]);
    assert_eq!(selected(cx).as_deref(), Some("a"));
}

/// A lost press on the marked bar's Clear button can survive the bar's redraws. Ending a sweep above the list
/// should clamp the range to the first row without delivering that earlier Clear click.
#[gpui::test]
fn a_sweep_release_on_the_bar_cannot_finish_an_older_clear_press(cx: &mut TestAppContext) {
    for cancelled in [false, true] {
        let (_fake, _list, cx) = open_list(cx, fleet(&["a", "b", "c"]));
        click_with(cx, "session-row-a", ctrl());
        assert_eq!(markers(cx), ["a"]);
        let selector = super::marking::CLEAR_ID;
        press(cx, selector, Modifiers::none());
        press(cx, "session-row-b", shift());
        drag_to(cx, "session-row-c", shift());
        assert_eq!(markers(cx), ["b", "c"]);
        drag_to(cx, selector, shift());
        assert_eq!(markers(cx), ["a", "b"], "the top edge clamped to a");
        let expected: &[_] = if cancelled {
            test_support::keys(cx, "escape");
            assert_eq!(
                markers(cx),
                ["a"],
                "Escape restored the marks before MouseUp"
            );
            &["a"]
        } else {
            &["a", "b"]
        };
        let at = test_support::center(cx, selector);
        release(cx, at, Modifiers::none());
        assert_eq!(markers(cx), expected, "the release did not activate Clear");
        assert_eq!(selected(cx).as_deref(), Some("a"));
    }
}

/// With no crossing, Shift still ranges from the open session. Dragging without Shift has no marking behavior.
#[gpui::test]
fn a_press_that_stays_in_its_row_is_a_shift_click_and_plain_drag_is_inert(cx: &mut TestAppContext) {
    let (_fake, _list, cx) = open_list(cx, fleet(&["a", "b", "c", "d"]));
    let at = press(cx, "session-row-c", shift());
    let at = at + point(px(2.), px(0.));
    cx.simulate_mouse_move(at, Some(MouseButton::Left), shift());
    release(cx, at, shift());
    assert_eq!(markers(cx), ["a", "b", "c"]);
    for mods in [Modifiers::none(), ctrl()] {
        press(cx, "session-row-b", mods);
        let at = drag_to(cx, "session-row-d", mods);
        release(cx, at, mods);
        assert_eq!(markers(cx), ["a", "b", "c"]);
        assert_eq!(selected(cx).as_deref(), Some("a"));
    }
}

/// Escape, a lost release and disappearance of the source all roll back to the same previously marked row. The
/// late MouseUp over the source after cancellation must never toggle a mark or open the source session.
#[gpui::test]
fn an_interrupted_sweep_restores_marks_and_rejects_the_late_release(cx: &mut TestAppContext) {
    for cancel in ["escape", "shift-escape", "lost release", "deleted source"] {
        let (mut fake, _list, cx) = open_list(cx, fleet(&["a", "b", "c", "d"]));
        click_with(cx, "session-row-d", ctrl());
        assert_eq!(markers(cx), ["d"]);
        press(cx, "session-row-b", shift());
        let at = drag_to(cx, "session-row-c", shift());
        assert_eq!(markers(cx), ["b", "c"], "{cancel}: the sweep started");
        match cancel {
            "escape" => {
                assert_eq!(
                    test_support::focused_id(cx).as_deref(),
                    Some("session-row-b")
                );
                test_support::keys(cx, "escape");
                cx.simulate_event(KeyDownEvent {
                    native_key: None,
                    keystroke: Keystroke::parse("escape").unwrap(),
                    is_held: true,
                    prefer_character_input: false,
                });
            }
            "shift-escape" => test_support::keys(cx, "shift-escape"),
            "lost release" => cx.simulate_mouse_move(at, None, shift()),
            "deleted source" => {
                fake.push(change(
                    listing_meta(&["a", "c", "d"], 3, false),
                    &[],
                    &["b"],
                ));
            }
            _ => unreachable!(),
        }
        test_support::settle(cx);
        assert_eq!(markers(cx), ["d"], "{cancel}: restored the earlier marks");
        let late = test_support::center(
            cx,
            if cancel == "deleted source" {
                "session-row-c"
            } else {
                "session-row-b"
            },
        );
        release(cx, late, shift());
        assert_eq!(markers(cx), ["d"], "{cancel}: the late release was inert");
        assert_eq!(selected(cx).as_deref(), Some("a"));
    }
}

/// A release is judged against the current listing even while dispatch still uses the frame that showed the source.
#[gpui::test]
fn a_sweep_restores_its_marks_if_the_store_loses_the_source_before_the_frame(
    cx: &mut TestAppContext,
) {
    use farhelm_helm_client::collections::decode_changes;
    for deleted in [true, false] {
        let (_fake, _list, cx) = open_list(
            cx,
            grouped_snapshot(
                &[
                    session("a").pinned(1),
                    session("b").grouped(10),
                    session("c"),
                    session("d"),
                ],
                vec![group(10, "Project", 1, false)],
            ),
        );
        click_with(cx, "session-row-d", ctrl());
        assert_eq!(markers(cx), ["d"]);
        press(cx, "session-row-b", shift());
        let at = drag_to(cx, "session-row-c", shift());
        assert_eq!(markers(cx), ["b", "c"]);
        assert_eq!(row_order(&frame(cx)), ["a", "b", "c", "d"]);
        let pushed = if deleted {
            change(listing_meta(&["a", "c", "d"], 3, false), &[], &["b"])
        } else {
            let mut meta = listing_meta(&["a", "b", "c", "d"], 4, false);
            meta.session_groups = vec![group(10, "Project", 1, true)];
            change(meta, &[], &[])
        };
        let pushed = decode_changes(pushed).unwrap();
        // Dispatch within the same update as the push so the platform driver cannot redraw between them.
        cx.update(|window, cx| {
            let store = crate::session::current_store(cx).unwrap();
            store.update(cx, |store, cx| store.apply(&pushed, cx));
            assert_eq!(row_order(window.driver_nodes()), ["a", "b", "c", "d"]);
            window.dispatch_event(
                MouseUpEvent {
                    button: MouseButton::Left,
                    position: at,
                    modifiers: shift(),
                    click_count: 1,
                }
                .to_platform_input(),
                cx,
            );
        });
        test_support::settle(cx);
        assert_eq!(markers(cx), ["d"], "deleted={deleted}");
    }
}

/// Crossing a collapsed header keeps the previous endpoint, and the hidden member is outside the range. A Shift
/// press on the header itself still toggles it and never becomes a sweep.
#[gpui::test]
fn a_sweep_crosses_collapsed_groups_without_marking_hidden_members(cx: &mut TestAppContext) {
    let (mut fake, list, cx) = open_list(
        cx,
        grouped_snapshot(
            &[
                session("a").pinned(1),
                session("b").pinned(2),
                session("c").grouped(10),
                session("d"),
            ],
            vec![group(10, "Folded", 1, true)],
        ),
    );
    assert_eq!(row_order(&frame(cx)), ["a", "b", "d"]);
    let row = test_support::center(cx, "session-row-b");
    cx.simulate_mouse_move(row, None, Modifiers::none());
    test_support::settle(cx);
    press(cx, "session-pin-grip-b:shown", shift());
    let at = drag_to(cx, "session-row-a", shift());
    assert!(
        cx.debug_bounds("session-pin-lifted-b").is_some(),
        "Shift on a grip still lifts the row"
    );
    assert!(markers(cx).is_empty());
    test_support::keys(cx, "escape");
    release(cx, at, shift());

    press(cx, "session-row-b", shift());
    drag_to(cx, "session-group-10-toggle", shift());
    assert_eq!(
        markers(cx),
        ["b"],
        "the header keeps the starting row as endpoint"
    );
    let at = drag_to(cx, "session-row-b", shift());
    release(cx, at, shift());
    assert_eq!(
        markers(cx),
        ["b"],
        "a return from the header remains a sweep"
    );

    press(cx, "session-row-a", shift());
    drag_to(cx, "session-row-b", shift());
    assert_eq!(markers(cx), ["a", "b"]);
    drag_to(cx, "session-group-10-toggle", shift());
    assert_eq!(markers(cx), ["a", "b"]);
    assert_eq!(
        row_order(&frame(cx)),
        ["a", "b", "d"],
        "sweeping over the header kept it collapsed"
    );
    let at = drag_to(cx, "session-row-d", shift());
    release(cx, at, shift());
    assert_eq!(markers(cx), ["a", "b", "d"]);
    assert!(!stored(&list, "c", cx));
    for command in fake.commands() {
        fake.reply(command.id, 200, None);
    }
    click_with(cx, "session-group-10-toggle", shift());
    assert!(
        fake.commands()
            .iter()
            .any(|cmd| cmd.path == "/api/session-groups/10"),
        "the header still sent its own command"
    );
    assert_eq!(markers(cx), ["a", "b", "d"]);
}

/// A new listing can move a different row under a stationary pointer. Recompute the live range after the new frame
/// and pick up the newly displayed member on a later release at the original endpoint's new position.
#[gpui::test]
fn a_sweep_includes_a_member_revealed_between_unchanged_endpoints(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(
        cx,
        grouped_snapshot(
            &[
                session("a").pinned(1),
                session("b").pinned(2),
                session("c").grouped(10),
                session("d"),
            ],
            vec![group(10, "Project", 1, true)],
        ),
    );
    assert_eq!(row_order(&frame(cx)), ["a", "b", "d"]);
    press(cx, "session-row-b", shift());
    let stationary = drag_to(cx, "session-row-d", shift());
    assert_eq!(markers(cx), ["b", "d"]);
    let mut meta = listing_meta(&["a", "b", "c", "d"], 4, false);
    meta.session_groups = vec![group(10, "Project", 1, false)];
    fake.push(change(meta, &[], &[]));
    test_support::settle(cx);
    assert_eq!(
        row_order(&frame(cx)),
        ["a", "b", "c", "d"],
        "the member became visible"
    );
    assert!(test_support::bounds(cx, "session-row-c").contains(&stationary));
    test_support::settle(cx);
    assert_eq!(
        markers(cx),
        ["b", "c"],
        "the pointer did not move but the endpoint did"
    );
    let at = test_support::center(cx, "session-row-d");
    release(cx, at, shift());
    assert_eq!(markers(cx), ["b", "c", "d"]);
}

/// GPUI may keep an offscreen focused row in the tree. Its clipped bounds say whether it is visible.
fn row_visible(cx: &mut VisualTestContext, id: &str) -> bool {
    frame(cx).iter().any(|node| {
        node.element_id.as_deref() == Some(id) && node.visible_bounds.size.height > px(0.)
    })
}

/// The shared edge timer brings an undrawn row under a stationary pointer. Clipping the original row cannot change
/// the range's anchor or turn the release into a click.
#[gpui::test]
fn a_sweep_scrolls_to_more_rows_while_its_anchor_is_clipped(cx: &mut TestAppContext) {
    let mut keys = vec!["a"];
    keys.extend((0..24).map(|index| -> &'static str { format!("filler-{index}").leak() }));
    keys.push("e");
    let (_fake, list, cx) = open_list(cx, fleet(&keys));
    assert!(!row_visible(cx, "session-row-e"));
    press(cx, "session-row-a", shift());
    let view = test_support::bounds(cx, "session-rows");
    let edge = point(view.center().x, view.bottom() + px(4.));
    cx.simulate_mouse_move(edge, Some(MouseButton::Left), shift());
    test_support::settle(cx);
    assert!(stored(&list, "a", cx));
    assert!(!stored(&list, "e", cx), "the endpoint starts out of sight");
    let reached = (0..100).any(|_| {
        test_support::advance(cx, Duration::from_millis(32));
        stored(&list, "e", cx)
    });
    assert!(
        reached,
        "the stationary pointer stopped extending the sweep at {:?}",
        row_order(&frame(cx))
    );
    assert!(!row_visible(cx, "session-row-a"), "the anchor is clipped");
    assert!(keys.iter().all(|key| stored(&list, key, cx)));
    release(cx, edge, shift());
    assert_eq!(selected(cx).as_deref(), Some("a"));
    assert!(stored(&list, "e", cx));
}

/// An active range keeps its displayed order even after the ordinary stillness deadline. Once the button comes up,
/// the waiting server order can take effect without changing the selection by key.
#[gpui::test]
fn a_sweep_keeps_the_order_held_after_pointer_stillness(cx: &mut TestAppContext) {
    let (mut fake, _list, cx) = open_list(cx, fleet(&["a", "b", "c", "d"]));
    assert_eq!(row_order(&frame(cx)), ["a", "b", "c", "d"]);
    press(cx, "session-row-b", shift());
    drag_to(cx, "session-row-c", shift());
    assert_eq!(markers(cx), ["b", "c"]);
    fake.push(change(
        listing_meta(&["a", "b", "d", "c"], 4, false),
        &[],
        &[],
    ));
    test_support::settle(cx);
    test_support::advance(cx, super::hold::STILLNESS + Duration::from_secs(1));
    assert_eq!(row_order(&frame(cx)), ["a", "b", "c", "d"]);
    let at = test_support::center(cx, "session-row-c");
    release(cx, at, shift());
    assert_eq!(row_order(&frame(cx)), ["a", "b", "d", "c"]);
    assert_eq!(markers(cx), ["b", "c"]);
}

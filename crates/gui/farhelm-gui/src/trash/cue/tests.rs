//! Tests of the archive cue in the real main view over the helm client's fake: a session row's Delete, sent at once
//! (Skip session confirmations on), answered as the helm answers it, and the Trash button's wiggle read back from the
//! button the cue reaches.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use gpui::{Modifiers, TestAppContext, VisualTestContext};
use serde_json::json;

use super::{Buttons, wiggle_angle};
use crate::session_list::menu_tests::snapshot;
use crate::session_list::tests::{change, listing_meta, open_main, session};
use crate::{test_support, ticker};

/// The main view over sessions `a` and `b`, with Skip session confirmations on, so a row's Delete is sent on the
/// click. Commands sent while it opens (preference writes, seen marks) are answered with success.
fn opened(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], true));
    let _ = sent_delete(&mut fake);
    (fake, cx)
}

/// Answer every command the window sent since the last call with success, except a Delete, whose id this returns.
fn sent_delete(fake: &mut FakeHelm) -> Option<u64> {
    let mut delete = None;
    for command in fake.commands() {
        if command.method == Method::Delete {
            delete = Some(command.id);
        } else {
            fake.reply(command.id, 200, None);
        }
    }
    delete
}

/// Delete `b` from its row's menu and return the Delete's id, unanswered.
fn delete_b(fake: &mut FakeHelm, cx: &mut VisualTestContext) -> u64 {
    test_support::click(cx, "session-menu-b");
    test_support::click(cx, "session-menu-delete");
    test_support::settle(cx);
    match sent_delete(fake) {
        Some(id) => id,
        None => panic!("b's Delete was sent"),
    }
}

/// Whether the window's Trash button still holds the wiggle's ticker handle.
fn wiggling(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| {
        let buttons: Vec<_> = cx
            .global::<Buttons>()
            .0
            .iter()
            .filter_map(|button| button.upgrade())
            .collect();
        assert_eq!(buttons.len(), 1, "the premise: one Trash button");
        buttons[0].read(cx).wiggling()
    })
}

/// The wiggle's turn on the window's Trash button now, `None` while it does not wiggle.
fn turn(cx: &mut VisualTestContext) -> Option<f32> {
    cx.update(|_, cx| {
        let buttons: Vec<_> = cx
            .global::<Buttons>()
            .0
            .iter()
            .filter_map(|button| button.upgrade())
            .collect();
        assert_eq!(buttons.len(), 1, "the premise: one Trash button");
        buttons[0].read(cx).wiggle_turn(cx)
    })
}

/// Why this matters: the wiggle is the old cue's motion, and the ticker is the only clock animation may use (ten ticks
/// a second). Specified: before it starts and once it has ended the can is upright (`None` after 900 ms); in between,
/// each tick's turn is the old keyframes (0, -16, 12, -8, 4, 0 degrees) eased out over 900 ms and sampled at the tick:
/// a swing to about -14 degrees after the first tick, small turns after, back to 0.
#[test]
fn the_wiggle_follows_the_old_keyframes_eased_out_over_900_ms() {
    let turns: Vec<Option<i32>> = (0..10)
        .map(|ticks| wiggle_angle(ticks).map(|turn| turn.round() as i32))
        .collect();
    assert_eq!(
        turns,
        [
            Some(0),
            Some(-14),
            Some(4),
            Some(3),
            Some(-7),
            Some(1),
            Some(3),
            Some(2),
            Some(0),
            None
        ]
    );
}

/// Why this matters: SPEC.md, "Managed checkouts": "Only the client that issued a session Delete plays an archive cue,
/// and only when the host confirms that a checkout moved", and the ticker drives it. Specified: b's Delete answered
/// with the helm's archived outcome starts the wiggle upright; each ticker tick turns the can as `wiggle_angle` says;
/// after 900 ms it has ended, the can is upright again and the button holds no ticker handle.
#[gpui::test]
fn a_confirmed_archive_from_this_window_wiggles_the_can(cx: &mut TestAppContext) {
    let (mut fake, cx) = opened(cx);
    let id = delete_b(&mut fake, cx);
    assert_eq!(
        turn(cx),
        None,
        "the premise: nothing wiggles before the answer"
    );
    fake.reply(id, 200, Some(json!({ "archived": true })));
    test_support::settle(cx);
    assert_eq!(turn(cx), Some(0.));
    for ticks in 1..=9 {
        test_support::advance(cx, ticker::TICK);
        assert_eq!(turn(cx), wiggle_angle(ticks), "after {ticks} ticks");
    }
    assert_eq!(turn(cx), None, "the wiggle ended");
    assert!(
        !wiggling(cx),
        "the ended wiggle let go of its ticker handle"
    );
}

/// Why this matters: ticks never reach an inactive window, so a wiggle that kept its handle there would stay tilted and
/// keep the ticker's timer running for as long as the window stays in the background. Specified: a wiggle under way
/// ends when the window goes inactive (upright, no handle held), and does not resume when the window is active again.
#[gpui::test]
fn a_wiggle_ends_when_the_window_goes_inactive(cx: &mut TestAppContext) {
    let (mut fake, cx) = opened(cx);
    let id = delete_b(&mut fake, cx);
    fake.reply(id, 200, Some(json!({ "archived": true })));
    test_support::settle(cx);
    test_support::advance(cx, ticker::TICK);
    assert!(wiggling(cx), "the premise: the wiggle is under way");
    cx.deactivate_window();
    test_support::settle(cx);
    assert_eq!(turn(cx), None, "upright once the window is inactive");
    assert!(!wiggling(cx), "no handle held in an inactive window");
    cx.update(|window, _| window.activate_window());
    test_support::settle(cx);
    test_support::advance(cx, ticker::TICK);
    assert_eq!(turn(cx), None, "it does not resume");
}

/// Why this matters: session disappearance alone is not proof a checkout moved (the protocol's `archived` flag says
/// so), and an agent's Delete, a bare Delete and a refused one must not look like an archive here. Specified: b's
/// Delete answered with a bare success (an empty outcome, and no body at all) or refused (409) wiggles nothing, and
/// neither does b's row disappearing from a pushed listing.
#[gpui::test]
fn nothing_but_a_confirmed_archive_wiggles(cx: &mut TestAppContext) {
    let (mut fake, cx) = opened(cx);
    for (status, body) in [
        (200, Some(json!({}))),
        (200, None),
        (409, Some(json!("session is busy"))),
    ] {
        let id = delete_b(&mut fake, cx);
        fake.reply(id, status, body.clone());
        test_support::settle(cx);
        assert_eq!(turn(cx), None, "{status} {body:?}");
        let _ = sent_delete(&mut fake);
    }
    fake.push(change(listing_meta(&["a"], 1, false), &[], &["b"]));
    test_support::settle(cx);
    assert_eq!(turn(cx), None, "a disappearance");
}

/// Why this matters: SPEC.md: "Reduced motion skips both parts", and the old cue played nothing for a button that was
/// not drawn. Specified: under reduced motion an archived answer starts no wiggle and holds no ticker handle; with motion back
/// but the Hosts panel shown in the list's place, so the Trash button is not drawn, an archived answer starts none
/// either, and none plays once the list, and the button, are back.
#[gpui::test]
fn reduced_motion_or_a_hidden_button_plays_nothing(cx: &mut TestAppContext) {
    let (mut fake, cx) = opened(cx);
    cx.update(|_, cx| ticker::set_reduced_motion(true, cx));
    let id = delete_b(&mut fake, cx);
    fake.reply(id, 200, Some(json!({ "archived": true })));
    test_support::settle(cx);
    test_support::advance(cx, ticker::TICK * 3);
    assert_eq!(turn(cx), None, "reduced motion starts no wiggle");
    assert!(!wiggling(cx), "and holds no ticker handle");

    cx.update(|_, cx| ticker::set_reduced_motion(false, cx));
    let id = delete_b(&mut fake, cx);
    test_support::click(cx, "hosts-panel-toggle");
    test_support::settle(cx);
    assert!(
        cx.debug_bounds(crate::trash::BUTTON_ID).is_none(),
        "the premise: the Trash button is not drawn"
    );
    fake.reply(id, 200, Some(json!({ "archived": true })));
    test_support::settle(cx);
    assert_eq!(turn(cx), None);
    test_support::click(cx, "hosts-panel-toggle");
    test_support::settle(cx);
    assert!(
        cx.debug_bounds(crate::trash::BUTTON_ID).is_some(),
        "the premise: the Trash button is drawn again"
    );
    assert_eq!(turn(cx), None, "and the cue it missed does not play late");
}

/// Why this matters: marking a session swaps the list's heading, the Trash button included, for the marked bar, so a
/// row's own Delete can archive a checkout while no Trash is drawn; the old app then played nothing, and checked that the
/// same cue does play once Trash is back (`bulk-session-actions.spec.ts`, "archive cues require a visible Trash
/// destination while marks remain"). A cue for a hidden button shows nothing and would hold the ticker meanwhile.
/// Specified: with a marked by the platform's toggle click, the Trash button is not drawn and b's archived Delete from
/// its own menu starts no wiggle; once Clear takes the marks away and the button is drawn again, still upright, b's next
/// archived Delete wiggles it.
#[gpui::test]
fn an_archive_while_sessions_are_marked_plays_nothing(cx: &mut TestAppContext) {
    let (mut fake, cx) = opened(cx);
    // SPEC.md's toggle gesture on this platform: Cmd+click on macOS, Ctrl+click elsewhere.
    let toggle = cx.update(|_, cx| Modifiers::secondary_key(cx));
    let row = test_support::center(cx, "session-row-a");
    cx.simulate_click(row, toggle);
    test_support::settle(cx);
    assert!(
        cx.debug_bounds(crate::trash::BUTTON_ID).is_none(),
        "the premise: the marked bar stands in the heading's place"
    );
    let id = delete_b(&mut fake, cx);
    fake.reply(id, 200, Some(json!({ "archived": true })));
    test_support::settle(cx);
    assert_eq!(turn(cx), None, "nothing wiggles toward a hidden Trash");
    assert!(!wiggling(cx), "and no ticker handle is held");

    test_support::click(cx, "session-marked-clear");
    test_support::settle(cx);
    assert!(
        cx.debug_bounds(crate::trash::BUTTON_ID).is_some(),
        "the premise: Clear brought the Trash button back"
    );
    assert_eq!(turn(cx), None, "the missed cue does not play late");
    let id = delete_b(&mut fake, cx);
    fake.reply(id, 200, Some(json!({ "archived": true })));
    test_support::settle(cx);
    assert_eq!(turn(cx), Some(0.), "the same answer wiggles a drawn Trash");
}

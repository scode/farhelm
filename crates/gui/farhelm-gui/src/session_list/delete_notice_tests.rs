//! Tests of the Delete notice band (`delete_notice`): what a Delete's reply puts there, what replaces it and what
//! leaves it, Dismiss and where focus goes after it, which changes of the window keep it, and an open row menu
//! following the rows the band moves.
//!
//! Everything runs in the real main view over the helm client's fake, as the menu's tests do (whose fixtures these
//! share), and reads what the window drew from GPUI's driver record of the frame. A Delete is started from a row's menu
//! (`session-menu-<key>`, then `session-menu-delete`) on an ended session with no tabs, which deletes without asking,
//! so each test reaches the reply in one step. The band is `session-delete-notice`, its words
//! `session-delete-notice-text` and its button `session-delete-notice-dismiss`. Replace's notice, which goes to the
//! same band, is tested beside Replace's other completions (`lifecycle_tests.rs`).

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::delete;
use super::delete_notice::{self, BAND_ID, DISMISS_ID, TEXT_ID};
use super::host_rail::tests::hover_help;
use super::menu_tests::{any_layer, drawn, ended, label_of, snapshot};
use super::tests::{open_main, selected, session};
use crate::test_support::{self, focused_id, frame, terminal_focused};
use crate::{focus, session_list::selection};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// What the supervisor writes when a deleted session's checkout could not be archived (`service/teardown.rs`), with a
/// path and a reason of the fixtures' own: the words the band must show as they are.
const NOT_ARCHIVED: &str = "The managed checkout at /work/bar-1 was not archived and stays where it is; Farhelm no \
                            longer manages it. Reason: its folder no longer matches the recorded root";

/// A second such notice, about another checkout, for the tests of what replaces the band.
const ALSO_NOT_ARCHIVED: &str = "The managed checkout at /work/baz-2 was not archived and may be there or in its \
                                 root's archive folder; Farhelm no longer manages it. Reason: the move's outcome is \
                                 unknown";

/// The window over the open session a and the ended sessions b, c and d, none with tabs, so each of the three
/// deletes at once from its menu.
fn window(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext) {
    let (fake, cx) = open_main(
        cx,
        snapshot(
            &[
                (session("a"), 0),
                (ended("b"), 0),
                (ended("c"), 0),
                (ended("d"), 0),
            ],
            false,
        ),
    );
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    (fake, cx)
}

/// Delete the session whose menu button is `button` from its menu and answer the one Delete it sends with success and
/// `reply`, as the helm's Delete route answers; every other command is answered with success at once, as the helm
/// answers a seen mark. Asserts that the Delete's completion ran (it records the session as this window's own
/// Delete), the premise of every assertion about what that completion left in the band, a missing band included.
fn delete_answered(
    fake: &mut FakeHelm,
    cx: &mut VisualTestContext,
    button: &'static str,
    reply: Option<Value>,
) {
    for command in fake.commands() {
        fake.reply(command.id, 200, None);
    }
    test_support::click(cx, button);
    test_support::click(cx, "session-menu-delete");
    let mut deletes = Vec::new();
    for command in fake.commands() {
        if command.method == Method::Delete {
            deletes.push(command.id);
        } else {
            fake.reply(command.id, 200, None);
        }
    }
    let [id] = deletes[..] else {
        panic!("one Delete was sent from {button}: {deletes:?}");
    };
    fake.reply(id, 200, reply);
    test_support::settle(cx);
    let key = button.strip_prefix("session-menu-").unwrap_or(button);
    assert!(
        cx.update(|_, cx| delete::deleted_here(key, cx)),
        "the premise: {key}'s Delete completed"
    );
}

/// The words the band shows in a fresh frame, or `None` when no band is drawn.
fn band_words(cx: &mut VisualTestContext) -> Option<String> {
    label_of(cx, TEXT_ID)
}

// ---- Tests ---------------------------------------------------------------------------------------------------------

/// SPEC.md, "Managed checkouts": a checkout that could not be archived "must never be silent: Delete completes with a
/// visible notice that the checkout was not archived". Before the band the GPUI app played only the trash cue and
/// dropped the notice. The band is the old `.delete-notice`, in its place under the status segments and above the
/// rows, with the helm's words as they are and a Dismiss named and helped as the old one was. Specified: before b's
/// Delete is answered no band is drawn; once the helm answers it with a not-archived notice, the band shows exactly the
/// helm's words, below the status segments and above a's row, and its Dismiss is a button named "Dismiss" whose hover
/// help is "Dismiss: hide this notice".
#[gpui::test]
fn a_delete_whose_checkout_was_not_archived_shows_the_helms_words_in_the_band(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = window(cx);
    assert!(!drawn(cx, BAND_ID), "the premise: no band before a Delete");

    delete_answered(
        &mut fake,
        cx,
        "session-menu-b",
        Some(json!({ "notice": NOT_ARCHIVED })),
    );
    assert_eq!(band_words(cx).as_deref(), Some(NOT_ARCHIVED));
    let band = test_support::bounds(cx, BAND_ID);
    assert!(
        band.top() >= test_support::bounds(cx, "session-status").bottom(),
        "the band sits under the status segments"
    );
    assert!(
        band.bottom() <= test_support::bounds(cx, "session-menu-a").top(),
        "the band sits above the rows"
    );
    let nodes = frame(cx);
    let dismiss = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(DISMISS_ID))
        .expect("the band draws its Dismiss");
    assert_eq!(dismiss.label.as_deref(), Some("Dismiss"));
    assert_eq!(dismiss.role, Some(gpui::Role::Button));
    assert_eq!(
        hover_help(cx, DISMISS_ID).as_deref(),
        Some("Dismiss: hide this notice")
    );
}

/// A Delete with nothing to report must not put up an empty band: the band exists for the one outcome SPEC.md says
/// must be visible, and showing it for every Delete would bury that one. An archived checkout reports itself through
/// the trash cue, not here. Specified: b's Delete answered with no body and c's answered with `archived` and no notice
/// leave no band drawn.
#[gpui::test]
fn a_delete_with_nothing_to_report_shows_no_band(cx: &mut TestAppContext) {
    let (mut fake, cx) = window(cx);
    delete_answered(&mut fake, cx, "session-menu-b", None);
    assert!(!drawn(cx, BAND_ID), "a Delete with no reply body");
    delete_answered(
        &mut fake,
        cx,
        "session-menu-c",
        Some(json!({ "archived": true })),
    );
    assert!(!drawn(cx, BAND_ID), "a Delete that archived its checkout");
}

/// The band holds one notice, the newest, as the old one did: a later Delete's notice takes its place, and a later
/// Delete with nothing to report leaves it alone (the old `publish(None)`), so an unrelated Delete never takes away a
/// notice the user has not read. Specified: b's Delete with a not-archived notice shows it; c's with no notice leaves
/// it shown; d's with another notice shows only d's words.
#[gpui::test]
fn a_later_notice_replaces_the_band_and_one_with_nothing_to_report_keeps_it(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = window(cx);
    delete_answered(
        &mut fake,
        cx,
        "session-menu-b",
        Some(json!({ "notice": NOT_ARCHIVED })),
    );
    assert_eq!(band_words(cx).as_deref(), Some(NOT_ARCHIVED), "the premise");

    delete_answered(&mut fake, cx, "session-menu-c", Some(json!({})));
    assert_eq!(band_words(cx).as_deref(), Some(NOT_ARCHIVED));

    delete_answered(
        &mut fake,
        cx,
        "session-menu-d",
        Some(json!({ "notice": ALSO_NOT_ARCHIVED })),
    );
    assert_eq!(band_words(cx).as_deref(), Some(ALSO_NOT_ARCHIVED));
}

/// SPEC.md, "Where focus rests": "whenever nothing holds focus it goes to the selected session's terminal", and "the
/// last notice was dismissed" is named among the causes. Dismiss holds focus when it is pressed (on Linux a click
/// focuses the control it lands on, the rule this test sets explicitly) and goes away with the band, so without the
/// coordinator's recovery focus would be left on nothing, where keys reach no terminal. Specified: with the band
/// showing, a press on Dismiss gives it focus; its release takes the band away, and focus is in the open session's
/// terminal.
#[gpui::test]
fn dismiss_takes_the_band_away_and_focus_goes_home_to_the_open_sessions_terminal(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = window(cx);
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    delete_answered(
        &mut fake,
        cx,
        "session-menu-b",
        Some(json!({ "notice": NOT_ARCHIVED })),
    );
    assert!(drawn(cx, BAND_ID), "the premise: the band shows");

    let at = test_support::center(cx, DISMISS_ID);
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    test_support::check(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(DISMISS_ID),
        "the premise: the press focused Dismiss"
    );
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    test_support::settle(cx);
    assert!(!drawn(cx, BAND_ID), "Dismiss took the band away");
    assert!(
        terminal_focused(cx),
        "focus went home to a's terminal, not to nothing"
    );
}

/// The band belongs to the UI session, as the rest of the list's per-session state does: what the user is looking at
/// does not change what a Delete reported, but a reset starts the window over (SPEC_impl.md, "One failure path: the
/// reset"), as the old app's reload did. Specified: with the band showing, opening c keeps it with the same words; a
/// lost channel and a new snapshot (a new UI session) leave no band drawn.
#[gpui::test]
fn the_band_survives_switching_sessions_and_a_reset_drops_it(cx: &mut TestAppContext) {
    let (mut fake, cx) = window(cx);
    delete_answered(
        &mut fake,
        cx,
        "session-menu-b",
        Some(json!({ "notice": NOT_ARCHIVED })),
    );
    assert_eq!(band_words(cx).as_deref(), Some(NOT_ARCHIVED), "the premise");

    cx.update(|_, cx| selection::choose("c", cx));
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("c"), "the premise: c is open");
    assert_eq!(band_words(cx).as_deref(), Some(NOT_ARCHIVED));

    fake.break_channel();
    cx.run_until_parked();
    fake.connect(snapshot(&[(session("a"), 0), (ended("c"), 0)], false));
    cx.run_until_parked();
    let nodes = test_support::frame(cx);
    let drawn_ids = |id: &str| {
        nodes
            .iter()
            .any(|node| node.element_id.as_deref() == Some(id))
    };
    assert!(
        drawn_ids("session-menu-a") && !drawn_ids("session-menu-d"),
        "the premise: the new snapshot's rows are drawn (d, never removed from the old one, is not in it)"
    );
    assert!(!drawn(cx, BAND_ID), "a reset drops the band");
}

/// SPEC.md, "Things changing underneath": "Focus follows the thing, not the position", and "Nothing closes a dialog,
/// menu, popup list or confirmation that it did not open". The old app closed an open row menu when the band moved it
/// (`popover-header-movement.spec.ts`), which the foundations' parity file records as superseded; here the menu is a
/// layer anchored to its button, and the band moving that button must leave it open beside it. Specified: with a's menu
/// open and focus on its first command, a notice put in the band moves a's menu button down; the menu is still open,
/// focus is still on that command, and the command moved down by the same distance as the button.
#[gpui::test]
fn an_open_row_menu_stays_open_and_follows_its_button_when_the_band_appears(
    cx: &mut TestAppContext,
) {
    let (_fake, cx) = window(cx);
    test_support::click(cx, "session-menu-a");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-rename"),
        "the premise: a's menu is open on its first command"
    );
    let button = test_support::bounds(cx, "session-menu-a").top();
    let item = test_support::bounds(cx, "session-menu-rename").top();

    cx.update(|_, cx| delete_notice::publish(Some(NOT_ARCHIVED.to_owned()), cx));
    test_support::settle(cx);
    assert!(drawn(cx, BAND_ID), "the premise: the band shows");
    let moved = test_support::bounds(cx, "session-menu-a").top() - button;
    assert!(
        moved > gpui::px(0.),
        "the premise: the band moved a's button down"
    );
    assert!(any_layer(cx), "the menu is still open");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));
    let followed = test_support::bounds(cx, "session-menu-rename").top() - item;
    assert!(
        (followed - moved).abs() < gpui::px(1.),
        "the menu moved with its button: {followed:?} against {moved:?}"
    );
}

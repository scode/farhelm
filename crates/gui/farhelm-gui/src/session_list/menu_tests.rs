//! Tests of a session row's actions menu, its Rename and its Delete: the menu's placement and focus, Rename opening the
//! editor for its row, the question and the precondition each request carries, what a pending or refused Delete shows,
//! the selection while a Delete runs, and the Delete shortcut.
//!
//! Everything runs in the real main view over the helm client's fake, as the list's own tests do, and reads what the
//! window sent from the fake and what it drew from GPUI's driver record of the frame. Controls are found by their ids:
//! a row's menu button is `session-menu-<key>`, the menu's Rename and Delete are `session-menu-rename` and
//! `session-menu-delete`, the Rename editor is `rename`, and the question's answers are `prompt-confirm` and
//! `prompt-cancel`.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{
    Collection, CollectionState, KeyedEntry, PREFERENCES_KEY, Preferences,
};
use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, PlatformInput, Point,
    TestAppContext, VisualTestContext,
};
use serde_json::{Value, json};

use super::tests::{Fixture, change, listing_meta, open_main, painted, selected, sent, session};
use crate::actions::{DELETE_SESSION_CHORD as DELETE_CHORD, RENAME_SESSION_SHORTCUT_NAME};
use crate::operations::OpKey;
use crate::test_support::{focused_id, frame, meta};
use crate::{layers, session, test_support};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// `key` as an ended session: its agent exited.
pub(super) fn ended(key: &'static str) -> Fixture {
    session(key).status(json!({"state": "exited", "exit_code": 0}))
}

/// `fixture`'s entry listing `tabs` terminal tabs. The helm's pinned fixture lists one, so every test says how many
/// it means.
pub(super) fn entry(fixture: &Fixture, tabs: usize) -> KeyedEntry {
    let mut entry = fixture.entry();
    entry.value["tabs"] = Value::Array(
        (0..tabs)
            .map(|tab| json!({ "id": format!("tab-{tab}") }))
            .collect(),
    );
    entry
}

/// A snapshot of `rows` (each with its tab count) in that order, with the first one open, and Skip session
/// confirmations as `skip` says.
pub(super) fn snapshot(rows: &[(Fixture, usize)], skip: bool) -> Vec<CollectionState> {
    let order: Vec<&str> = rows.iter().map(|(fixture, _)| fixture.key).collect();
    let preferences = Preferences {
        visit_history: order.iter().take(1).map(|key| key.to_string()).collect(),
        skip_session_confirmations: Some(skip),
        ..Preferences::default()
    };
    vec![
        CollectionState {
            collection: Collection::Sessions,
            meta: listing_meta(&order, rows.len() as u64, false),
            entries: rows
                .iter()
                .map(|(fixture, tabs)| entry(fixture, *tabs))
                .collect(),
        },
        CollectionState {
            collection: Collection::Preferences,
            meta: meta(&[PREFERENCES_KEY]),
            entries: vec![KeyedEntry {
                key: PREFERENCES_KEY.into(),
                value: serde_json::to_value(preferences).unwrap(),
            }],
        },
    ]
}

/// Push `rows` as changed, under the complete listing `order`.
pub(super) fn push(
    fake: &mut FakeHelm,
    order: &[&str],
    rows: &[(Fixture, usize)],
    cx: &mut VisualTestContext,
) {
    let mut changes = change(listing_meta(order, order.len() as u64, false), &[], &[]);
    changes.upsert = rows
        .iter()
        .map(|(fixture, tabs)| entry(fixture, *tabs))
        .collect();
    fake.push(changes);
    test_support::settle(cx);
}

/// The Deletes the window sent since the last call, as (id, path). Every other command is answered with success at
/// once, as the helm answers a preference write or a seen mark, so a test can answer its Deletes in the order the
/// fake requires; a test that has a Delete waiting reads later commands with `sent` instead.
fn deletes(fake: &mut FakeHelm) -> Vec<(u64, String)> {
    let mut found = Vec::new();
    for FakeCommand {
        id, method, path, ..
    } in fake.commands()
    {
        if method == Method::Delete {
            found.push((id, path));
        } else {
            fake.reply(id, 200, None);
        }
    }
    found
}

/// The paths of `deletes`.
fn paths(deletes: &[(u64, String)]) -> Vec<&str> {
    deletes.iter().map(|(_, path)| path.as_str()).collect()
}

/// Whether anything was drawn under `selector` in the last frame.
pub(super) fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.debug_bounds(selector).is_some()
}

/// Whether any dialog, menu or question is open.
pub(super) fn any_layer(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| layers::any_open(window, cx))
}

/// The label of the node drawn under the element id `id`.
pub(super) fn label_of(cx: &mut VisualTestContext, id: &str) -> Option<String> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .and_then(|node| node.label.map(|label| label.to_string()))
}

/// Open the menu of the row whose menu button is `button`, and choose Delete, by clicks.
fn choose_delete(cx: &mut VisualTestContext, button: &'static str) {
    test_support::click(cx, button);
    test_support::click(cx, "session-menu-delete");
    test_support::settle(cx);
}

/// A left click at `at`, delivered inside the caller's window update, so several can land with no frame drawn between
/// them.
pub(super) fn click_at(at: Point<Pixels>) -> [PlatformInput; 2] {
    [
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        }),
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: 1,
        }),
    ]
}

// ---- The menu ------------------------------------------------------------------------------------------------------

/// SPEC.md, "Opening and closing" and "Inside a dialog or menu, focus stays inside": a menu the user opened starts on
/// its first command, keeps Tab inside, and Escape gives focus back to the button that opened it. Opening a row's menu
/// is not opening its session, which would switch what the user is looking at. Specified: clicking b's menu button
/// opens the menu below the button with focus on its first command, Rename, its header names b and says "Codex ·
/// Running", a is still the open session and nothing is sent; Tab moves through Replace, Stop and Delete, in the old
/// menu's order (the separators between their groups take no focus), and then wraps to Rename, staying in the menu;
/// Escape closes the menu and focus is on b's button.
#[gpui::test]
fn the_menu_opens_below_its_button_holds_focus_and_leaves_the_selection(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    deletes(&mut fake);

    test_support::click(cx, "session-menu-b");
    let button = test_support::bounds(cx, "session-menu-b");
    let menu = test_support::bounds(cx, "session-menu");
    assert!(
        menu.top() >= button.bottom(),
        "the menu is below its button: {menu:?} under {button:?}"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));
    assert_eq!(
        label_of(cx, "session-menu-summary-b").as_deref(),
        Some("Codex · Running")
    );
    assert!(painted(&frame(cx)).iter().any(|text| text == "b"));
    assert_eq!(selected(cx).as_deref(), Some("a"));
    assert_eq!(sent(&mut fake), []);

    for next in [
        "session-menu-replace",
        "session-menu-stop",
        "session-menu-delete",
        "session-menu-rename",
    ] {
        test_support::keys(cx, "tab");
        assert_eq!(focused_id(cx).as_deref(), Some(next));
    }
    test_support::keys(cx, "escape");
    assert!(!any_layer(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-b"));
}

/// SPEC.md: a menu goes away "when the thing it is about is gone (a deleted session's open menu, say)", and focus
/// never stays inside something that is no longer drawn. This is the popover-movement acceptance case the layer
/// manager left to the first real menu. Specified: with b's menu open, a pushed listing without b closes the menu.
#[gpui::test]
fn a_pushed_removal_of_the_row_closes_its_menu(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    test_support::click(cx, "session-menu-b");
    assert!(any_layer(cx), "the premise: the menu is open");

    fake.push(change(listing_meta(&["a"], 1, false), &[], &["b"]));
    test_support::settle(cx);
    test_support::redraw(cx);
    assert!(!any_layer(cx));
    assert!(!drawn(cx, "session-menu"));
}

// ---- Rename --------------------------------------------------------------------------------------------------------

/// The row menu is one of Rename's two openers (SPEC.md, "Lifecycle operations": "A list rename opens in a modal editor
/// owned by the list rather than by the row-actions popup"). It must rename the row it was opened from, not the open
/// session, and give focus back to the menu's button when the editor closes. Specified: with a open, Enter on b's
/// Rename closes the menu and opens the editor; Escape closes it with focus on b's button; opened again, Enter saves,
/// sending b's own title to b's rename route.
#[gpui::test]
fn the_menus_rename_opens_the_editor_for_its_row(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    deletes(&mut fake);

    test_support::click(cx, "session-menu-b");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-rename"),
        "the premise: the menu starts on Rename"
    );
    test_support::keys(cx, "enter");
    assert!(drawn(cx, "rename"), "the editor opened");
    assert!(!drawn(cx, "session-menu"), "and the menu closed");
    test_support::keys(cx, "escape");
    assert!(!any_layer(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-b"));

    test_support::click(cx, "session-menu-b");
    test_support::keys(cx, "enter enter");
    assert_eq!(
        sent(&mut fake),
        [("/api/sessions/b/rename".to_owned(), json!({ "title": "b" }))]
    );
}

/// SPEC.md: a pending request "refuses another lifecycle action" on its session, and a Rename sent behind a Delete
/// would race it. Specified: with Skip session confirmations on, b's Delete is sent and left unanswered; b's menu then
/// draws Rename unavailable, and choosing it opens no editor and sends nothing.
#[gpui::test]
fn the_menus_rename_is_refused_while_a_delete_of_its_row_is_in_flight(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], true));
    deletes(&mut fake);
    choose_delete(cx, "session-menu-b");
    assert_eq!(
        paths(&deletes(&mut fake)),
        ["/api/sessions/b"],
        "the premise: b's Delete is in flight"
    );

    test_support::click(cx, "session-menu-b");
    test_support::click(cx, "session-menu-rename");
    // What tells an unavailable item from an enabled one whose open was refused: the enabled one closes the menu.
    assert!(
        drawn(cx, "session-menu"),
        "the unavailable Rename ignored the click; the menu is still open"
    );
    assert!(!drawn(cx, "rename"));
    assert_eq!(sent(&mut fake), []);
}

/// SPEC.md, "Desktop window chrome": the Rename shortcut appears "only on the open session's row-menu item", the one
/// row the shortcut acts on; naming it on another row would suggest it renames that one. Specified: with a open, the
/// help of Rename in a's menu, shown after the hover delay, holds the shortcut's name, and in b's menu it does not.
#[gpui::test]
fn only_the_open_rows_rename_names_the_shortcut(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    deletes(&mut fake);
    let mut help_in_menu_of = |button: &'static str| {
        test_support::click(cx, button);
        let rename = test_support::center(cx, "session-menu-rename");
        cx.simulate_mouse_move(rename, None, Modifiers::none());
        test_support::advance(cx, layers::HOVER_DELAY);
        let help = cx
            .update(|window, cx| layers::shown_tooltip(window, cx))
            .map(|shown| shown.text.to_string());
        test_support::keys(cx, "escape escape");
        help.expect("the premise: Rename's help is showing")
    };
    let open = help_in_menu_of("session-menu-a");
    let other = help_in_menu_of("session-menu-b");
    assert!(open.contains(RENAME_SESSION_SHORTCUT_NAME), "{open}");
    assert!(!other.contains(RENAME_SESSION_SHORTCUT_NAME), "{other}");
}

// ---- The question and the request ----------------------------------------------------------------------------------

/// SPEC.md, "Opening and closing": "from a row's menu, Delete then Enter deletes", and a live session's question says
/// it kills the agent, quotes the title on a line of its own and, for a managed checkout, says what happens to it. A
/// click on the question's empty space keeps focus on Confirm (SPEC.md gives the row Delete question that rule), so the
/// Enter after it still answers what the user was looking at. Specified, by keys after the menu opens: Up to Delete
/// (wrapping from Rename to the last command) and Enter asks with focus on Confirm and sends nothing; the question's
/// lines are the consequence, the quoted title and the checkout line; a click on the question's empty space leaves
/// focus on Confirm; Enter then sends the Delete with no precondition flag and closes the question.
#[gpui::test]
fn delete_then_enter_asks_about_a_live_session_and_enter_deletes_it(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0)], false));
    deletes(&mut fake);
    test_support::click(cx, "session-menu-a");
    test_support::keys(cx, "up");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-delete"),
        "the premise: Up from the first command reaches Delete"
    );
    test_support::keys(cx, "enter");
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    let texts = painted(&frame(cx));
    for line in [
        "Still running — deleting kills the agent:",
        "The managed checkout stays while another session uses it. When its last session is deleted, its folder \
         moves to the trash; no files are deleted.",
    ] {
        assert!(texts.iter().any(|text| text == line), "{line}: {texts:?}");
    }
    assert_eq!(
        label_of(cx, "session-delete-title-a").as_deref(),
        Some("\"a\"")
    );
    assert_eq!(deletes(&mut fake), []);

    // Inside the question's 8-pixel padding: empty space.
    let corner = test_support::bounds(cx, "prompt").origin;
    cx.simulate_click(
        gpui::point(corner.x + gpui::px(3.), corner.y + gpui::px(3.)),
        Modifiers::none(),
    );
    test_support::check(cx);
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    test_support::keys(cx, "enter");
    assert_eq!(paths(&deletes(&mut fake)), ["/api/sessions/a"]);
    assert!(!any_layer(cx));
}

/// The precondition is what makes a stale row safe: SPEC.md, "Confirmed 2026-10-01", has the supervisor refuse a
/// Delete when the session has more alive than the click showed. The old app's `delete-precondition.spec.ts` pinned
/// three cases. Specified: a running session asks and sends no flag; an ended session with two tabs asks about the
/// tabs and sends `only_if_agent_ended`; an ended session with no tabs asks nothing and sends `only_if_nothing_alive`
/// at once.
#[gpui::test]
fn each_delete_carries_the_precondition_of_what_its_row_showed(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(
        cx,
        snapshot(
            &[(session("a"), 0), (ended("b"), 2), (ended("c"), 0)],
            false,
        ),
    );
    deletes(&mut fake);

    choose_delete(cx, "session-menu-a");
    assert!(any_layer(cx), "a running session asks");
    test_support::click(cx, "prompt-confirm");
    choose_delete(cx, "session-menu-b");
    assert!(
        painted(&frame(cx)).iter().any(|text| text
            == "2 terminal tabs are still open, and deleting closes them with everything running in them; delete \
                anyway:"),
        "an ended session's tabs are what it asks about"
    );
    test_support::click(cx, "prompt-confirm");
    choose_delete(cx, "session-menu-c");
    assert!(!any_layer(cx), "nothing alive asks nothing");
    assert_eq!(
        paths(&deletes(&mut fake)),
        [
            "/api/sessions/a",
            "/api/sessions/b?only_if_agent_ended=true",
            "/api/sessions/c?only_if_nothing_alive=true",
        ]
    );
}

/// SPEC.md: with **Skip session confirmations** on, a row Delete "runs on the first click through the ordinary action
/// path, with consent from the state shown by that render". Specified: with the preference on, Delete on a running
/// session with a tab opens no question and sends the Delete with no flag.
#[gpui::test]
fn skipped_confirmations_delete_on_the_click_with_the_rows_guard(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 1)], true));
    deletes(&mut fake);
    choose_delete(cx, "session-menu-a");
    assert!(!any_layer(cx));
    assert_eq!(paths(&deletes(&mut fake)), ["/api/sessions/a"]);
}

/// The prompt's guarantee at the real question (SPEC_impl.md, "Questions: the prompt"): the answer acts on what the
/// question showed, and only the first answer runs, even when the session changes under the open question and both
/// answers land before a redraw. Specified: with a's question open, a pushed change ends a's agent, and Cancel then
/// Confirm delivered with no frame between them sends nothing; with b's question open and b ended the same way,
/// Confirm then Cancel sends b's Delete with no flag, the guard of the running row the question was drawn from.
///
/// At this real question the second click finds the question closed and its view already released, which is what
/// stops it here; the prompt's own first-answer guard, which matters when both answers reach a prompt that is still
/// alive, is tested by `prompt_tests::a_confirmation_acts_only_on_what_it_showed` (a hand mutant that removed the
/// guard failed only that test).
#[gpui::test]
fn a_question_answers_once_for_what_it_showed_even_after_the_session_changed(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    deletes(&mut fake);

    for (key, button, order, expected) in [
        (
            "a",
            "session-menu-a",
            ["prompt-cancel", "prompt-confirm"],
            vec![],
        ),
        (
            "b",
            "session-menu-b",
            ["prompt-confirm", "prompt-cancel"],
            vec!["/api/sessions/b"],
        ),
    ] {
        choose_delete(cx, button);
        assert!(any_layer(cx), "the premise: {key}'s question is open");
        push(&mut fake, &["a", "b"], &[(ended(key), 0)], cx);
        assert!(any_layer(cx), "{key}: an ended session is not gone");
        let points = [
            test_support::center(cx, order[0]),
            test_support::center(cx, order[1]),
        ];
        cx.update(|window, cx| {
            for at in points {
                for event in click_at(at) {
                    window.dispatch_event(event, cx);
                }
            }
        });
        test_support::settle(cx);
        assert_eq!(paths(&deletes(&mut fake)), expected, "{key}: {order:?}");
        assert!(!any_layer(cx), "{key}: the question closed");
    }
}

/// SPEC.md: "an open lifecycle confirmation … refuses another lifecycle action … on that session", and the row's
/// button must not reopen a menu over its own question. Specified: with a's question open, clicking a's menu button
/// leaves the question open, opens no menu and sends nothing.
#[gpui::test]
fn the_menu_button_is_inert_while_its_question_is_open(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0)], false));
    deletes(&mut fake);
    choose_delete(cx, "session-menu-a");
    assert!(drawn(cx, "prompt"), "the premise: the question is open");

    test_support::click(cx, "session-menu-a");
    test_support::redraw(cx);
    assert!(drawn(cx, "prompt"));
    assert!(!drawn(cx, "session-menu"));
    assert_eq!(sent(&mut fake), []);
}

// ---- A pending and a refused Delete --------------------------------------------------------------------------------

/// SPEC.md, "Lifecycle operations": while a Delete runs the row shows "a spinner and what is happening in place of its
/// title", the session's own competing actions are refused, other sessions act normally, and "Opening the session
/// remains possible, including while its Delete is pending". The old row hid its title meanwhile and kept it for
/// assistive technology. Specified: with a's Delete sent and unanswered, a's row paints "Stopping agent…" and not its
/// title, which stays its open control's accessible name; a's menu Delete does nothing, not even close the menu; b's
/// menu still asks; clicking a's row opens a, and the Delete shortcut over it asks nothing; no second Delete is sent.
#[gpui::test]
fn a_pending_delete_shows_on_its_row_refuses_another_and_leaves_the_rest_alone(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("b"), 0), (session("a"), 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    deletes(&mut fake);
    choose_delete(cx, "session-menu-a");
    test_support::click(cx, "prompt-confirm");
    assert_eq!(paths(&deletes(&mut fake)), ["/api/sessions/a"]);

    let texts = painted(&frame(cx));
    assert!(texts.iter().any(|text| text == "Stopping agent…"));
    assert!(!texts.iter().any(|text| text == "a"), "the title gives way");
    assert_eq!(label_of(cx, "session-row-a").as_deref(), Some("a"));
    choose_delete(cx, "session-menu-a");
    assert!(!drawn(cx, "prompt"), "a's own Delete is refused");
    assert!(
        drawn(cx, "session-menu"),
        "and is not even taken: its menu stays open"
    );
    test_support::keys(cx, "escape");

    choose_delete(cx, "session-menu-b");
    assert!(drawn(cx, "prompt"), "b's Delete still asks");
    test_support::click(cx, "prompt-cancel");

    test_support::click(cx, "session-row-a");
    assert_eq!(selected(cx).as_deref(), Some("a"));
    test_support::keys(cx, DELETE_CHORD);
    assert!(!drawn(cx, "prompt"), "nor does the shortcut ask about it");
    // The writes for opening a are behind a's unanswered Delete, so they are read, not answered.
    assert!(
        sent(&mut fake)
            .iter()
            .all(|(path, _)| !path.starts_with("/api/sessions/a?") && path != "/api/sessions/a"),
        "no second Delete"
    );
}

/// SPEC.md: "a pending request refuses another lifecycle action", whatever the request: a session being renamed is not
/// deleted under it, which would also make the Delete's Confirm send nothing (the operation store refuses a second
/// request). Specified: with a Rename of a sent and unanswered, a's menu Delete opens nothing and leaves the menu open,
/// and the shortcut over the open session a asks nothing; no Delete is sent.
#[gpui::test]
fn any_pending_request_refuses_the_rows_delete(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0)], false));
    deletes(&mut fake);
    cx.update(|_, cx| {
        let operations = session::current_operations(cx).unwrap();
        let rename = super::rename::command("a", "renamed").unwrap();
        operations.update(cx, |operations, cx| {
            operations
                .start(&OpKey::session("a"), rename, cx, |_, _| {})
                .unwrap()
        });
    });
    assert!(
        sent(&mut fake)
            .iter()
            .any(|(path, _)| path == "/api/sessions/a/rename"),
        "the premise: a's Rename is in flight"
    );

    choose_delete(cx, "session-menu-a");
    assert!(!drawn(cx, "prompt"));
    assert!(
        drawn(cx, "session-menu"),
        "Delete is unavailable in the menu"
    );
    test_support::keys(cx, "escape");
    test_support::keys(cx, DELETE_CHORD);
    assert!(!drawn(cx, "prompt"));
    assert_eq!(sent(&mut fake), []);
}

/// SPEC.md: "A failure or partial result appears on the source session's row", and the operation store keeps it "until
/// the next operation on that session starts". Specified: a refused Delete paints the helm's words on the row and
/// takes the spinner away; they stay through an unrelated pushed change; a new Delete of the row asks again and, once
/// confirmed, the words are gone and the spinner is back.
#[gpui::test]
fn a_refusal_shows_on_the_row_until_the_next_delete_starts(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    deletes(&mut fake);
    choose_delete(cx, "session-menu-a");
    test_support::click(cx, "prompt-confirm");
    let [(id, _)] = deletes(&mut fake)[..] else {
        panic!("one Delete was sent");
    };
    fake.reply(id, 409, Some(json!("the agent is busy")));
    test_support::settle(cx);
    let texts = painted(&frame(cx));
    assert!(texts.iter().any(|text| text == "the agent is busy"));
    assert!(!texts.iter().any(|text| text == "Stopping agent…"));

    push(
        &mut fake,
        &["a", "b"],
        &[(session("b").title("renamed"), 0)],
        cx,
    );
    assert!(
        painted(&frame(cx))
            .iter()
            .any(|text| text == "the agent is busy")
    );

    choose_delete(cx, "session-menu-a");
    assert!(drawn(cx, "prompt"), "a refused Delete asks again");
    test_support::click(cx, "prompt-confirm");
    let texts = painted(&frame(cx));
    assert!(!texts.iter().any(|text| text == "the agent is busy"));
    assert!(texts.iter().any(|text| text == "Stopping agent…"));
}

/// SPEC.md, "Session list": "Automatic selection after Delete waits until every admitted Delete has settled … and
/// never opens a session that is still being deleted". The helm pushes the removal before it answers, so a list that
/// picked the next session on the removal would open it while the Delete was still on its way. Specified: with a open
/// and its Delete sent, a pushed listing without a leaves nothing open; once the Delete is answered, b opens and is
/// remembered.
#[gpui::test]
fn the_next_session_opens_only_once_the_delete_has_settled(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    deletes(&mut fake);
    choose_delete(cx, "session-menu-a");
    test_support::click(cx, "prompt-confirm");
    let [(id, _)] = deletes(&mut fake)[..] else {
        panic!("one Delete was sent");
    };

    fake.push(change(listing_meta(&["b"], 1, false), &[], &["a"]));
    test_support::settle(cx);
    assert_eq!(selected(cx), None, "nothing opens while the Delete runs");

    fake.reply(id, 200, None);
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("b"));
    assert!(
        sent(&mut fake)
            .iter()
            .any(|(path, body)| path == "/api/preferences" && body["last_selected"] == "b")
    );
}

// ---- The Delete shortcut -------------------------------------------------------------------------------------------

/// SPEC.md, "Desktop window chrome": the shortcut "invokes the open session's header Delete action with its ordinary
/// confirmation and busy rules. With no session open it does nothing", and "A visible modal keeps keyboard ownership".
/// With no header yet, it asks the row's question beside the open session's menu button. Specified: the chord asks
/// about the open session a, below a's button, with focus on Confirm and nothing sent; the chord again while the
/// question is open changes nothing; Enter deletes a.
#[gpui::test]
fn the_shortcut_asks_about_the_open_session_and_waits_while_a_question_is_open(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    deletes(&mut fake);
    test_support::keys(cx, DELETE_CHORD);
    let button = test_support::bounds(cx, "session-menu-a");
    let question = test_support::bounds(cx, "prompt");
    assert!(question.top() >= button.bottom(), "asked below a's button");
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    assert_eq!(deletes(&mut fake), []);

    test_support::keys(cx, DELETE_CHORD);
    assert!(drawn(cx, "prompt"));
    assert_eq!(deletes(&mut fake), []);

    test_support::keys(cx, "enter");
    assert_eq!(paths(&deletes(&mut fake)), ["/api/sessions/a"]);
}

/// The shortcut's two quiet cases: with no session open it does nothing (and the key must still be taken, as a
/// once-per-press shortcut's must), and a menu that is open keeps the keyboard. Specified: over an empty fleet the
/// chord opens nothing and sends nothing; with b's menu open, the chord leaves the menu as the only layer and sends
/// nothing.
#[gpui::test]
fn the_shortcut_does_nothing_with_no_session_open_or_a_menu_open(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[], false));
    test_support::keys(cx, DELETE_CHORD);
    assert!(!any_layer(cx));
    assert_eq!(sent(&mut fake), []);

    fake.connect(snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    test_support::settle(cx);
    deletes(&mut fake);
    test_support::click(cx, "session-menu-b");
    test_support::keys(cx, DELETE_CHORD);
    assert!(drawn(cx, "session-menu"));
    assert!(!drawn(cx, "prompt"));
    assert_eq!(deletes(&mut fake), []);
}

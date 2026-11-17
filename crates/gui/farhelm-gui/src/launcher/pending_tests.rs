//! Tests of the window around a launch whose create is still out, in the real main view over the helm client's fake:
//! the rest of the window keeps working, and what the reply then does respects what the user did meanwhile. Ported
//! from the old app's `nonblocking-launch` spec at reference commit `24bc0580`, whose point was that a pending New is
//! not a modal: SPEC.md, "Creation", has the launcher close while its create is out so the page is not isolated, and
//! its reply "leaves focus, panels, menus and dialogs as the user left them" once they have moved on.
//!
//! Also here: a YOLO question for a launch the user moved on from, which waits behind its notice and, once answered,
//! sends the same draft wherever the user went meanwhile.
//!
//! The fleet is the sidebar's menu fixture (`session_list::menu_tests::snapshot`) with Skip session confirmations on,
//! so a row menu's Stop and Delete go out without a question, plus host1, the one connected host, for the launch. Every
//! launch is the smallest complete draft ([`command_draft`]), or its YOLO-asserted twin. The fake answers commands only
//! when told, and in the order they were sent, as the helm does, so each test holds the create's reply until the moment
//! it means to test, and nothing sent after it can be answered first.

use farhelm_helm_client::Method;
use farhelm_proto::ui_channel::{Collection, CollectionState};
use gpui::TestAppContext;
use serde_json::json;

use super::launch_tests::{command_draft, created_as, creates, launched_over, selected};
use super::view_tests::{fixture_hosts, name_of};
use super::yolo_tests::{refuse_for_yolo, yolo_command_draft};
use crate::session_list::menu_tests::{drawn, ended, snapshot};
use crate::session_list::tests::{Fixture, change, listing_meta, session};
use crate::test_support::{self, any_open, focused_id, settle, terminal_focused};

// ---- Fixtures --------------------------------------------------------------------------------------------------------

/// The fleet: `rows` (the first one open, none with tabs) with Skip session confirmations on, `visited` as the helm's
/// visit history, newest first, and host1, the one connected host.
fn fleet(rows: &[Fixture], visited: &[&str]) -> Vec<CollectionState> {
    let rows: Vec<(Fixture, usize)> = rows.iter().map(|row| (row.clone(), 0)).collect();
    let mut state = snapshot(&rows, true);
    for collection in &mut state {
        if collection.collection == Collection::Preferences {
            collection.entries[0].value["visit_history"] = json!(visited);
        }
    }
    state.push(fixture_hosts(&[0]));
    state
}

// ---- Tests ---------------------------------------------------------------------------------------------------------

/// A pending New holds nothing but its own draft: with its create out, another session opens, that session's Stop goes
/// out beside the create, and a row menu opened meanwhile stays open when the create succeeds (nonblocking-launch.spec's
/// "a pending New keeps one immutable draft while sidebar, switcher and terminal work" and "a row menu opened during New
/// survives its accepted completion"; the terminal holding the keyboard is in the next test). Why:
/// the old app's New was modal while its create was out, so a slow host froze the whole window. Specified: with a open
/// and the create out, a click on b's row opens b; b's menu Stop sends one `POST /api/sessions/b/stop`; c's menu opened;
/// then the create succeeds as d: c's menu is still open with the keyboard where it was, and b is still open.
#[gpui::test]
fn the_window_keeps_working_while_a_create_is_out(cx: &mut TestAppContext) {
    let (mut helm, cx, create) = launched_over(
        cx,
        fleet(&[session("a"), session("b"), ended("c")], &["a"]),
        command_draft(),
    );
    test_support::click(cx, "session-row-b");
    assert_eq!(selected(cx).as_deref(), Some("b"), "another session opened");
    test_support::click(cx, "session-menu-b");
    test_support::click(cx, "session-menu-stop");
    cx.run_until_parked();
    let stops: Vec<_> = helm
        .commands()
        .into_iter()
        .filter(|command| command.method == Method::Post && command.path == "/api/sessions/b/stop")
        .collect();
    assert_eq!(stops.len(), 1, "b's Stop went out beside the create");
    test_support::click(cx, "session-menu-c");
    assert!(drawn(cx, "session-menu"), "the premise: c's menu is open");
    let in_menu = focused_id(cx);
    assert!(
        in_menu
            .as_deref()
            .is_some_and(|id| id.starts_with("session-menu-")),
        "the premise: the menu holds the keyboard: {in_menu:?}"
    );
    created_as(
        &mut helm,
        cx,
        create.id,
        &["a", "b", "c", "d"],
        session("d"),
    );
    assert!(
        drawn(cx, "session-menu"),
        "the menu stayed open through the success"
    );
    assert_eq!(focused_id(cx), in_menu, "and kept the keyboard");
    assert_eq!(
        selected(cx).as_deref(),
        Some("b"),
        "the selection stayed where the user left it"
    );
}

/// A YOLO question for a launch the user moved on from waits behind the refusal's notice like any refusal, and its
/// answer then sends the same draft again, to the same host and folder although the user opened another session
/// meanwhile; the session it makes opens, since the answer was the user's last action (nonblocking-launch.spec's "a held
/// YOLO refusal restores the modal question and retries the same draft" and "New opens after its YOLO answer when no
/// later input intervenes"). Why: a question that opened by itself would take the keyboard from whatever the user went
/// on to, and SPEC.md, "Creation", has a launch keep the destination it was submitted with. Specified: with a open and
/// the YOLO-asserted create out, a click opens b, on host9; the helm's YOLO refusal then opens nothing and offers Open
/// launcher, which brings the question back; "Start YOLO session anyway" sends one create whose body is the first one's
/// with `confirm_yolo` set; its success as d opens d.
#[gpui::test]
fn a_yolo_question_the_user_moved_on_from_waits_and_its_answer_resends_the_draft(
    cx: &mut TestAppContext,
) {
    // b runs on host9 as host9's recorded install, so New beside b would launch there: the retry staying on host1
    // in `~` is the destination kept, not New's defaults happening to agree.
    let mut state = fleet(&[session("a"), session("b").host(9)], &["a"]);
    for collection in &mut state {
        match collection.collection {
            Collection::Sessions => {
                let b = collection.entries.iter_mut().find(|entry| entry.key == "b");
                b.expect("the fleet lists b").value["host_identity"] = json!("identity9");
            }
            Collection::Hosts => *collection = fixture_hosts(&[0, 8]),
            _ => {}
        }
    }
    let (mut helm, cx, first) = launched_over(cx, state, yolo_command_draft());
    test_support::click(cx, "session-row-b");
    assert_eq!(
        selected(cx).as_deref(),
        Some("b"),
        "the premise: the user moved on to b"
    );
    assert_eq!(
        cx.update(|_, cx| super::entry::new_prefill(cx))
            .and_then(|draft| draft.host),
        Some(9),
        "the premise: New beside b would launch on host9"
    );
    refuse_for_yolo(&mut helm, cx, first.id);
    assert!(
        !any_open(cx),
        "nothing opened over what the user went on to"
    );
    assert_eq!(
        name_of(cx, "notice-open-launcher").as_deref(),
        Some("Open launcher")
    );
    test_support::click(cx, "notice-open-launcher");
    assert_eq!(
        name_of(cx, "prompt-confirm").as_deref(),
        Some("Start YOLO session anyway"),
        "the question came back with the launcher"
    );
    test_support::click(cx, "prompt-confirm");
    let sent = creates(&mut helm, cx);
    let [retry] = &sent[..] else {
        panic!("one answer sent one create, not {}", sent.len());
    };
    let mut expected = first.body.clone().expect("a create carries a body");
    expected["confirm_yolo"] = json!(true);
    assert_eq!(
        retry.body.as_ref(),
        Some(&expected),
        "the same draft and destination, with consent"
    );
    created_as(&mut helm, cx, retry.id, &["a", "b", "d"], session("d"));
    assert_eq!(
        selected(cx).as_deref(),
        Some("d"),
        "the answered launch opened its session"
    );
}

/// A Delete of the open session sent while a launch is out picks the next open session by the ordinary rules once it
/// settles, and the launch's success, which comes first, takes no part in it (nonblocking-launch.spec's "Delete selects
/// its successor while New is held and New completion keeps it"). The old case's order of replies cannot happen: the UI
/// channel answers in order, so a Delete sent while a create is out is answered after it (SPEC_impl.md, "The UI
/// channel"). What it guarded can: a launch the user moved on from opens nothing, and the Delete's pick is not bent
/// toward the session the launch made. Specified: with a open, b visited before it, and the create out, a's menu Delete
/// is sent; the create succeeds as d, the newest session, leaving a open; the helm drops a and then answers the Delete:
/// b, the most recently visited one still listed, is open, not d.
#[gpui::test]
fn a_delete_while_a_launch_is_out_picks_by_the_ordinary_rules(cx: &mut TestAppContext) {
    let (mut helm, cx, create) = launched_over(
        cx,
        fleet(&[ended("a"), ended("b"), ended("c")], &["a", "b"]),
        command_draft(),
    );
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    test_support::click(cx, "session-menu-a");
    test_support::click(cx, "session-menu-delete");
    cx.run_until_parked();
    let waiting = helm.commands();
    assert_eq!(
        waiting
            .iter()
            .filter(|command| command.method == Method::Delete)
            .count(),
        1,
        "the Delete went out while the create is out"
    );
    created_as(
        &mut helm,
        cx,
        create.id,
        &["a", "b", "c", "d"],
        session("d").created(200),
    );
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "the launch the user moved on from opened nothing"
    );
    // The helm pushes a Delete's change before its reply (SPEC_impl.md, "The UI channel"); then everything sent after
    // the create is answered, in order, the Delete among it.
    helm.push(change(
        listing_meta(&["b", "c", "d"], 3, false),
        &[],
        &["a"],
    ));
    settle(cx);
    for command in waiting {
        helm.reply(command.id, 200, None);
    }
    settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some("b"),
        "the most recently visited session still listed is open, not the launch's newer one"
    );
}

/// A launch's success leaves the keyboard in the session the user switched to while it was out: the new session
/// appears without taking focus (nonblocking-launch.spec's "New leaves typing in the selected session after switching
/// during its request"). Why: the keys of a half-typed line meant for b would otherwise land in the new session's
/// terminal. Specified: with a open, the keyboard is a's terminal's as soon as the launcher hides; with the create out,
/// a click on b's row opens b with its terminal focused; the create succeeds as d: b is still open and its terminal
/// still has the keyboard; and New, opened then, starts with no name rather than the launched draft's (the old spec's "New
/// preserves Hosts and reports its success without retaining its launcher").
#[gpui::test]
fn a_launchs_success_leaves_the_keyboard_in_the_session_switched_to(cx: &mut TestAppContext) {
    let (mut helm, cx, create) = launched_over(cx, fleet(&[session("a"), session("b")], &["a"]), {
        let mut draft = command_draft();
        draft.edit_name("launched-one".into());
        draft
    });
    assert!(
        terminal_focused(cx),
        "the hidden launcher gave the keyboard back to a's terminal"
    );
    test_support::click(cx, "session-row-b");
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b opened");
    assert!(
        terminal_focused(cx),
        "the premise: b's terminal has the keyboard"
    );
    created_as(&mut helm, cx, create.id, &["a", "b", "d"], session("d"));
    assert_eq!(selected(cx).as_deref(), Some("b"));
    assert!(terminal_focused(cx), "the keyboard stayed in b's terminal");
    // The launcher went with its success: the next New starts from New's own defaults, not the launched draft.
    assert!(cx.update(super::entry::open_new));
    settle(cx);
    assert_eq!(
        super::view_tests::draft(cx).name.value(),
        "",
        "the next New starts fresh"
    );
}

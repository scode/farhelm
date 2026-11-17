//! Tests of a row's Stop and Replace: when each asks, the preconditions each request carries for what the row showed,
//! what a pending one shows and refuses, what a refusal or a success leaves on the row, and where a finished Replace
//! leaves the selection and focus.
//!
//! Everything runs in the real main view over the helm client's fake, as the menu's own tests do (whose fixtures and
//! helpers these share), and reads what the window sent from the fake and what it drew from GPUI's driver record of
//! the frame. A row's menu button is `session-menu-<key>`, its Replace and Stop are `session-menu-replace` and
//! `session-menu-stop`, a question's answers are `prompt-confirm` and `prompt-cancel`, and the lines under a row are
//! named `session-outcome-<key>` (a refusal or a notice) and `session-replace-progress-label-<key>` (a long Replace).
//! The YOLO question a Replace meets (the launcher's) names its answers `prompt-cancel`, `prompt-alternative-0` (the
//! permanent answer) and `prompt-confirm` (the one-off answer), and its reason `yolo-reason`.

use std::time::Duration;

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use gpui::{
    AppContext as _, Modifiers, Pixels, ScrollDelta, ScrollEvent, TestAppContext,
    VisualTestContext, point, px,
};
use gpui_component::Root;
use serde_json::{Value, json};

use super::delete_notice;
use super::menu_tests::{any_layer, click_at, drawn, ended, entry, label_of, lit, push, snapshot};
use super::outcome::PROGRESS_SHOWS_AFTER;
use super::tests::{
    Fixture, change, launcher_drawn, listing_meta, open_main, painted, selected, sent, session,
};
use crate::focus::Mark;
use crate::main_view::MainView;
use crate::operations::{HelmMarker, OpKey};
use crate::test_support::{focused_id, frame, started, terminal_focused};
use crate::{layers, session, test_support};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// Whether `command` is a Stop or a Replace.
fn is_lifecycle(command: &FakeCommand) -> bool {
    command.method == Method::Post
        && (command.path.ends_with("/stop") || command.path.ends_with("/replace"))
}

/// The Stops and Replaces the window sent since the last call, as (id, path, body), left unanswered for the test to
/// answer. Every other command is answered with success at once, as the helm answers a preference write or a seen mark;
/// the fake answers in order, so that works only while nothing sent before it waits, and a test with a Stop or Replace
/// waiting reads later commands with `sent` instead.
fn pending(fake: &mut FakeHelm) -> Vec<(u64, String, Value)> {
    let mut found = Vec::new();
    for command in fake.commands() {
        if is_lifecycle(&command) {
            found.push((command.id, command.path, command.body.unwrap_or_default()));
        } else {
            fake.reply(command.id, 200, None);
        }
    }
    found
}

/// [`pending`]'s requests without their ids.
fn requests(sent: Vec<(u64, String, Value)>) -> Vec<(String, Value)> {
    sent.into_iter()
        .map(|(_, path, body)| (path, body))
        .collect()
}

/// The Stops and Replaces the window sent since the last call, as (path, body), with every command answered in order:
/// each Stop and Replace as a success with an empty reply (which leaves nothing on its row and selects nothing), the
/// rest as for [`pending`]. For tests about what is sent, so that nothing waits behind a request they do not answer.
fn answered(fake: &mut FakeHelm) -> Vec<(String, Value)> {
    let mut found = Vec::new();
    for command in fake.commands() {
        if is_lifecycle(&command) {
            fake.reply(command.id, 200, Some(json!({})));
            found.push((command.path, command.body.unwrap_or_default()));
        } else {
            fake.reply(command.id, 200, None);
        }
    }
    found
}

/// A Stop of `key` with these preconditions, as the window sends it.
fn stop(key: &str, refuse_if_tabs_open: bool, refuse_if_agent_running: bool) -> (String, Value) {
    (
        format!("/api/sessions/{key}/stop"),
        json!({
            "refuse_if_tabs_open": refuse_if_tabs_open,
            "refuse_if_agent_running": refuse_if_agent_running,
        }),
    )
}

/// A plain Replace of `key` with these flags of its source's Delete guard, as the window sends it: the shared client
/// body with nothing else set, so no Replace with, no YOLO consent and no retry key (the body has no field for one).
pub(crate) fn replace(
    key: &str,
    only_if_nothing_alive: bool,
    only_if_agent_ended: bool,
) -> (String, Value) {
    (
        format!("/api/sessions/{key}/replace"),
        json!({
            "confirm_yolo": false,
            "with": null,
            "only_if_nothing_alive": only_if_nothing_alive,
            "only_if_agent_ended": only_if_agent_ended,
        }),
    )
}

/// [`replace`] sent again with the user's YOLO consent: the same body with `confirm_yolo` set and nothing else changed.
pub(crate) fn consented(
    key: &str,
    only_if_nothing_alive: bool,
    only_if_agent_ended: bool,
) -> (String, Value) {
    let (path, mut body) = replace(key, only_if_nothing_alive, only_if_agent_ended);
    body["confirm_yolo"] = json!(true);
    (path, body)
}

/// The helm's words for a YOLO refusal of a Replace on the fixtures' host, as its `YoloNeedsConfirmation` writes them.
/// Written for the command line: the YOLO question never shows them.
pub(crate) const YOLO_WORDS: &str = "buildbox asks before YOLO launches, and this launch is YOLO or uses the agent's own \
                                     setting: nothing was started.";

/// The helm refuses command `id` for YOLO, as it does for a YOLO launch on a host that asks first: a 409 carrying the
/// helm's YOLO marker.
pub(crate) fn refuse_for_yolo(fake: &mut FakeHelm, cx: &mut VisualTestContext, id: u64) {
    fake.reply_marked(
        id,
        409,
        vec![HelmMarker::YoloConfirmation],
        Some(json!(YOLO_WORDS)),
    );
    test_support::settle(cx);
}

/// Whether a fresh frame draws the element `id` inside the element `ancestor`, by the frame's own parent links.
pub(crate) fn drawn_inside(cx: &mut VisualTestContext, id: &str, ancestor: &str) -> bool {
    let nodes = frame(cx);
    let Some(mut at) = nodes
        .iter()
        .position(|node| node.element_id.as_deref() == Some(id))
    else {
        return false;
    };
    while let Some(parent) = nodes[at].parent {
        if nodes[parent].element_id.as_deref() == Some(ancestor) {
            return true;
        }
        at = parent;
    }
    false
}

/// Open the menu of the row whose menu button is `button` and choose `item`, by clicks.
fn choose(cx: &mut VisualTestContext, button: &'static str, item: &'static str) {
    test_support::click(cx, button);
    test_support::click(cx, item);
    test_support::settle(cx);
}

/// The hover help of the open question's Cancel, as shown once the pointer has rested on it for the hover delay.
fn cancel_help(cx: &mut VisualTestContext) -> Option<String> {
    let cancel = test_support::center(cx, "prompt-cancel");
    cx.simulate_mouse_move(cancel, None, Modifiers::none());
    test_support::advance(cx, layers::HOVER_DELAY);
    cx.update(|window, cx| layers::shown_tooltip(window, cx))
        .map(|shown| shown.text.to_string())
}

/// Every line of text painted in a fresh frame.
fn texts(cx: &mut VisualTestContext) -> Vec<String> {
    painted(&frame(cx))
}

// ---- When each asks, and what each sends ---------------------------------------------------------------------------

/// SPEC.md, "Lifecycle operations": Stop asks only "With tabs open", and the old row never asked just because the agent
/// was alive; and the request carries what the click showed, so that the supervisor refuses a tab the user did not see
/// or an agent the command line restarted ("Confirmed 2026-10-01"). Specified: a running row and an ended row without
/// tabs stop on the click, asking nothing; a running row with one tab asks "Stop the agent and close 1 terminal tab?"
/// with its title quoted, focus on Confirm and Cancel's help "Cancel: keep the agent and terminal tabs running" (the
/// old row's); an ended row with two tabs asks "Close 2 terminal tabs?"; the four Stops carry, in order, no tabs shown
/// and a live agent, no tabs and an ended agent, tabs and a live agent, and tabs and an ended agent.
#[gpui::test]
fn stop_asks_only_about_tabs_and_carries_what_its_row_showed(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(
        cx,
        snapshot(
            &[
                (session("a"), 0),
                (ended("b"), 0),
                (session("c"), 1),
                (ended("d"), 2),
            ],
            false,
        ),
    );
    pending(&mut fake);

    choose(cx, "session-menu-a", "session-menu-stop");
    assert!(!any_layer(cx), "a live agent without tabs asks nothing");
    choose(cx, "session-menu-b", "session-menu-stop");
    assert!(!any_layer(cx), "an ended agent without tabs asks nothing");

    choose(cx, "session-menu-c", "session-menu-stop");
    assert!(drawn(cx, "prompt"), "a row with a tab asks");
    assert!(
        texts(cx)
            .iter()
            .any(|text| text == "Stop the agent and close 1 terminal tab?")
    );
    assert_eq!(
        label_of(cx, "session-stop-title-c").as_deref(),
        Some("\"c\"")
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    assert_eq!(
        cancel_help(cx).as_deref(),
        Some("Cancel: keep the agent and terminal tabs running"),
        "Cancel's help is the old row question's"
    );
    test_support::click(cx, "prompt-confirm");

    choose(cx, "session-menu-d", "session-menu-stop");
    assert!(
        texts(cx)
            .iter()
            .any(|text| text == "Close 2 terminal tabs?")
    );
    test_support::click(cx, "prompt-confirm");

    assert_eq!(
        answered(&mut fake),
        [
            stop("a", true, false),
            stop("b", true, true),
            stop("c", false, false),
            stop("d", false, true),
        ]
    );
}

/// SPEC.md: "Replace keeps its confirmation whatever the agent is doing", unlike Delete, which goes ahead on a row with
/// nothing alive; and the source's removal carries Delete's precondition for what the question showed. Cancel must send
/// nothing. Specified: Replace on a running row asks with the old consequence and the quoted title, focus on Confirm
/// and Cancel's help "Cancel: keep this session as it is" (the old row's), and Cancel sends nothing; asked again and
/// confirmed, and then on an ended row with a tab and an ended row without one (each asking too), the three Replaces
/// carry no flag, `only_if_agent_ended`, and `only_if_nothing_alive`.
#[gpui::test]
fn replace_always_asks_and_carries_the_delete_guard_of_its_row(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(
        cx,
        snapshot(
            &[(session("a"), 0), (ended("b"), 1), (ended("c"), 0)],
            false,
        ),
    );
    pending(&mut fake);

    choose(cx, "session-menu-a", "session-menu-replace");
    assert!(drawn(cx, "prompt"));
    assert!(texts(cx).iter().any(|text| text
        == "Still running — replacing kills the agent and discards the conversation; a fresh session with the same \
            settings takes its place:"));
    assert_eq!(
        label_of(cx, "session-replace-title-a").as_deref(),
        Some("\"a\"")
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    assert_eq!(
        cancel_help(cx).as_deref(),
        Some("Cancel: keep this session as it is"),
        "Cancel's help is the old row question's"
    );
    test_support::click(cx, "prompt-cancel");
    assert!(!any_layer(cx));
    assert_eq!(pending(&mut fake), [], "Cancel sends nothing");

    choose(cx, "session-menu-a", "session-menu-replace");
    test_support::click(cx, "prompt-confirm");
    choose(cx, "session-menu-b", "session-menu-replace");
    assert!(drawn(cx, "prompt"), "an ended row with a tab asks");
    test_support::click(cx, "prompt-confirm");
    choose(cx, "session-menu-c", "session-menu-replace");
    assert!(drawn(cx, "prompt"), "so does a row with nothing alive");
    test_support::click(cx, "prompt-confirm");

    assert_eq!(
        answered(&mut fake),
        [
            replace("a", false, false),
            replace("b", false, true),
            replace("c", true, false),
        ]
    );
}

/// SPEC.md: with **Skip session confirmations** on, row Stop and row Replace each run "on the first click through the
/// ordinary action path, with consent from the state shown by that render", under the same guards. Specified: with the
/// preference on, Stop on a running row with a tab and Replace on an ended row with a tab open no question, and send a
/// Stop authorizing the tabs and a Replace guarded by the ended agent.
#[gpui::test]
fn skipped_confirmations_stop_and_replace_on_the_click_with_their_rows_guards(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 1), (ended("b"), 1)], true));
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-stop");
    assert!(!any_layer(cx));
    choose(cx, "session-menu-b", "session-menu-replace");
    assert!(!any_layer(cx));
    assert_eq!(
        answered(&mut fake),
        [stop("a", false, false), replace("b", false, true)]
    );
}

/// The prompt's guarantee at the real Stop and Replace questions (SPEC_impl.md, "Questions: the prompt"): whatever
/// runs acts on the state the question froze when it opened, even after the session changed under it, and only the
/// first of two answers delivered with no redraw between them runs. Each question here is opened on a running row,
/// then the helm pushes that row with its agent ended and no tabs, which would have given other preconditions.
/// Specified: Stop's Cancel then Confirm sends nothing; Stop's Confirm then Cancel sends the Stop of the row as asked
/// (tabs shown, a live agent); the same two orders on Replace send nothing, then the Replace with no flag.
#[gpui::test]
fn an_answer_sends_the_preconditions_its_question_was_drawn_from(cx: &mut TestAppContext) {
    let rows = [
        (session("a"), 1),
        (session("b"), 1),
        (session("c"), 0),
        (session("d"), 0),
    ];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, false));
    pending(&mut fake);

    for (key, button, item, order, expected) in [
        (
            "a",
            "session-menu-a",
            "session-menu-stop",
            ["prompt-cancel", "prompt-confirm"],
            vec![],
        ),
        (
            "b",
            "session-menu-b",
            "session-menu-stop",
            ["prompt-confirm", "prompt-cancel"],
            vec![stop("b", false, false)],
        ),
        (
            "c",
            "session-menu-c",
            "session-menu-replace",
            ["prompt-cancel", "prompt-confirm"],
            vec![],
        ),
        (
            "d",
            "session-menu-d",
            "session-menu-replace",
            ["prompt-confirm", "prompt-cancel"],
            vec![replace("d", false, false)],
        ),
    ] {
        choose(cx, button, item);
        assert!(any_layer(cx), "the premise: {key}'s question is open");
        push(&mut fake, &["a", "b", "c", "d"], &[(ended(key), 0)], cx);
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
        assert_eq!(answered(&mut fake), expected, "{key}: {order:?}");
        assert!(!any_layer(cx), "{key}: the question closed");
    }
}

// ---- While a Stop or a Replace runs --------------------------------------------------------------------------------

/// SPEC.md: "A Stop shows a spinner and **Stopping…** on its row", and "Each session excludes its own competing
/// actions … Opening the session remains possible" while other sessions act normally. Unlike Delete's, a Stop's row
/// keeps its title, since the session stays. Specified: with a's Stop sent and unanswered, a's row paints "Stopping…"
/// and its title; in a's menu, Rename, Clone, Replace with…, Replace, Stop and Delete each ignore a click (the menu
/// stays open; SPEC.md names Clone among what a pending request refuses); b's Replace still asks; a click on a's row
/// opens a; nothing but the opening's preference write and seen mark is sent. Once the Stop is answered, "Stopping…"
/// is gone and a's Replace asks again.
#[gpui::test]
fn a_pending_stop_shows_beside_the_title_and_refuses_only_its_own_sessions_actions(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("b"), 0), (session("a"), 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-stop");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Stop was sent");
    };

    let painted = texts(cx);
    assert!(painted.iter().any(|text| text == "Stopping…"));
    assert!(painted.iter().any(|text| text == "a"), "the title stays");
    test_support::click(cx, "session-menu-a");
    for item in [
        "session-menu-rename",
        "session-menu-clone",
        "session-menu-replace-with",
        "session-menu-replace",
        "session-menu-stop",
        "session-menu-delete",
    ] {
        test_support::click(cx, item);
        assert!(
            drawn(cx, "session-menu"),
            "{item} is unavailable: the menu is still open"
        );
        assert!(!drawn(cx, "prompt"), "{item} asks nothing");
    }
    test_support::keys(cx, "escape");

    choose(cx, "session-menu-b", "session-menu-replace");
    assert!(drawn(cx, "prompt"), "b's Replace still asks");
    test_support::click(cx, "prompt-cancel");
    test_support::click(cx, "session-row-a");
    assert_eq!(selected(cx).as_deref(), Some("a"));
    // The opening's writes (and the session query naming a) wait behind a's Stop, so they are read, not answered.
    for (path, _) in sent(&mut fake) {
        assert!(
            path == "/api/preferences"
                || path == "/api/sessions/a/seen"
                || path == farhelm_proto::ui_channel::SESSION_QUERY_PATH,
            "only the opening's writes were sent: {path}"
        );
    }

    fake.reply(id, 200, Some(json!({})));
    test_support::settle(cx);
    assert!(!texts(cx).iter().any(|text| text == "Stopping…"));
    choose(cx, "session-menu-a", "session-menu-replace");
    assert!(
        drawn(cx, "prompt"),
        "a's Replace asks once its Stop settled"
    );
}

/// An unavailable command stays focusable (`components/menu_item.rs`), so a menu gone wholly unavailable under a
/// pending request still takes its keys and Escape, but it must neither run nor wear the highlight's fill ([`lit`]),
/// which would make it read as ready. The old `sidebar.spec.ts` "a reopened busy menu stays navigable while refusing to
/// act" and `menu-highlight.spec.ts` "host menu hover transfers focus and disabled actions never fill or run" (reference
/// commit `24bc0580`); the latter used the host menu's Update, and the item is one component every menu shares. Run
/// under macOS's click rule, so focus is back on a's button only because the menu returns it there.
///
/// Specified, with b open: a's menu opens on Rename, which alone wears the fill (the premise that the fill is read at
/// all); choosing Stop by a click closes the menu with focus on a's button and sends a's one Stop, left unanswered; a
/// click opens the menu again on Rename, now with no fill anywhere; Down moves focus through Mark unread, Pin to top,
/// Clone, Replace with…, Replace and Stop, none of them filled; Enter on Stop leaves the menu open with focus on Stop;
/// the pointer entering Replace moves focus there, still unfilled, and a click on it leaves the menu open and asks
/// nothing; Home goes back to Rename; Escape closes the menu with focus on a's button; no other Stop or Replace is sent.
#[gpui::test]
fn a_busy_menu_walks_its_unavailable_commands_without_filling_or_running_them(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("b"), 0), (session("a"), 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    pending(&mut fake);
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    let none: [&str; 0] = [];

    test_support::click(cx, "session-menu-a");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));
    assert_eq!(
        lit(cx, "session-menu"),
        ["session-menu-rename"],
        "the premise: an available command's highlight is painted"
    );
    test_support::click(cx, "session-menu-stop");
    let [(_, path, _)] = &pending(&mut fake)[..] else {
        panic!("one Stop was sent");
    };
    assert_eq!(path, "/api/sessions/a/stop");
    assert!(!any_layer(cx), "choosing Stop closed the menu");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-a"));

    test_support::click(cx, "session-menu-a");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-rename"),
        "the busy menu still opens on its first command"
    );
    assert_eq!(lit(cx, "session-menu"), none, "Rename is unavailable");
    for next in [
        "session-menu-seen",
        "session-menu-pin",
        "session-menu-clone",
        "session-menu-replace-with",
        "session-menu-replace",
        "session-menu-stop",
    ] {
        test_support::keys(cx, "down");
        assert_eq!(focused_id(cx).as_deref(), Some(next));
        assert_eq!(lit(cx, "session-menu"), none, "{next} is unavailable");
    }
    test_support::keys(cx, "enter");
    assert!(
        drawn(cx, "session-menu"),
        "Enter on Stop left the menu open"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-stop"));

    let replace = test_support::center(cx, "session-menu-replace");
    cx.simulate_mouse_move(replace, None, Modifiers::none());
    test_support::settle(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-replace"),
        "the pointer moves the highlight onto an unavailable command too"
    );
    assert_eq!(lit(cx, "session-menu"), none, "and it stays unfilled");
    test_support::click(cx, "session-menu-replace");
    assert!(drawn(cx, "session-menu"), "the click left the menu open");
    assert!(!drawn(cx, "prompt"), "and asked nothing");

    test_support::keys(cx, "home");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));
    test_support::keys(cx, "escape");
    assert!(!any_layer(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-a"));
    assert!(
        fake.commands().iter().all(|command| !is_lifecycle(command)),
        "no other Stop or Replace was sent"
    );
}

/// The old `session-action-completion.spec.ts`, "Stop settles before its held status refresh": Stop's own reply ends
/// its hold on the session, whether or not any listing has shown the agent end, so Clone (and New terminal, which reads
/// the same session key) works at once. A hold that waited for the status would keep both refused while the agent
/// winds down. Specified: with b's Stop unanswered, b's row says "Stopping…" and Clone from b's menu opens nothing;
/// once the helm answers with a partial-Stop notice, and with no push since, "Stopping…" is gone, the notice shows on
/// b's row and Clone opens the launcher.
#[gpui::test]
fn a_stops_reply_alone_frees_its_sessions_clone(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    pending(&mut fake);
    choose(cx, "session-menu-b", "session-menu-stop");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Stop was sent");
    };
    assert!(
        texts(cx).iter().any(|text| text == "Stopping…"),
        "the premise"
    );
    choose(cx, "session-menu-b", "session-menu-clone");
    assert!(!launcher_drawn(cx), "the premise: Clone waits for the Stop");
    test_support::keys(cx, "escape");

    let notice = "Terminal 2 could not be closed: fixture partial Stop";
    fake.reply(id, 200, Some(json!({ "notice": notice })));
    test_support::settle(cx);
    assert!(!texts(cx).iter().any(|text| text == "Stopping…"));
    assert_eq!(
        label_of(cx, "session-outcome-b"),
        Some(format!("Stop: {notice}"))
    );
    choose(cx, "session-menu-b", "session-menu-clone");
    assert!(launcher_drawn(cx), "Clone opened the launcher");
}

/// SPEC.md: "After about two seconds waiting for the reply, the sidebar … share a spinner, the host and elapsed
/// seconds, updated about once a second. The status disappears on the reply." The old `LaunchProgress` showed nothing
/// before two whole seconds, so a quick reply adds no height to the row. Time is the executor's clock, moved by the
/// test. Specified: with a's Replace confirmed and unanswered, nothing shows 1.9 seconds in; at two seconds the row says
/// "Replacing on buildbox…" (the row's host, as the helm names it) and "2 s", then "3 s" and "4 s" a second apart, while
/// a's Stop in its menu is unavailable; once the helm refuses, the line is gone and the refusal shows instead.
#[gpui::test]
fn a_replace_says_after_two_seconds_how_long_it_has_waited(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0)], false));
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-replace");
    test_support::click(cx, "prompt-confirm");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Replace was sent");
    };
    let progress = |cx: &mut VisualTestContext| label_of(cx, "session-replace-progress-label-a");

    test_support::advance(cx, PROGRESS_SHOWS_AFTER - Duration::from_millis(100));
    assert_eq!(progress(cx), None, "nothing before two seconds");
    test_support::advance(cx, Duration::from_millis(100));
    assert_eq!(progress(cx).as_deref(), Some("Replacing on buildbox…"));
    assert!(texts(cx).iter().any(|text| text == "2 s"));
    for seconds in ["3 s", "4 s"] {
        test_support::advance(cx, Duration::from_secs(1));
        assert!(texts(cx).iter().any(|text| text == seconds), "{seconds}");
    }
    test_support::click(cx, "session-menu-a");
    test_support::click(cx, "session-menu-stop");
    assert!(
        drawn(cx, "session-menu"),
        "a's Stop is unavailable meanwhile"
    );
    test_support::keys(cx, "escape");

    fake.reply(id, 409, Some(json!("the host refused this operation")));
    test_support::settle(cx);
    assert_eq!(progress(cx), None, "the line goes with the reply");
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some("Replace: the host refused this operation")
    );
}

/// SPEC.md, "Session list": "Pending Stop batches do not delay automatic selection", and only Deletes hold it back; a
/// selection that went away while a Stop or Replace of another session runs must not wait for them. Specified: with b
/// open and a Stop of a and a Replace of c in flight, a pushed complete listing without b opens a, the first of the
/// newest (b was the only visit). Both requests are left unanswered, and the store is asked that they are still in
/// flight, since a test whose requests had settled would pass whatever pending work did to the choice.
#[gpui::test]
fn a_pending_stop_or_replace_holds_no_automatic_choice_back(cx: &mut TestAppContext) {
    let rows = [(session("b"), 0), (session("a"), 0), (session("c"), 0)];
    let (mut fake, cx) = open_main(cx, snapshot(&rows, true));
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-stop");
    choose(cx, "session-menu-c", "session-menu-replace");
    assert_eq!(
        requests(pending(&mut fake)),
        [stop("a", true, false), replace("c", false, false)],
    );
    assert!(
        cx.update(|_, cx| super::stop::in_flight("a", cx) && super::replace::in_flight("c", cx)),
        "the premise: both are in flight"
    );

    fake.push(change(listing_meta(&["a", "c"], 2, false), &[], &["b"]));
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("a"));
}

// ---- What a settled Stop or Replace leaves -------------------------------------------------------------------------

/// SPEC.md: "A failure or partial result appears on the source session's row", and the old app kept it there, named by
/// its operation ("Stop: …", "Replace: …"), until the next operation on the session started; the open menu covers the
/// row's copy, so the menu shows it too. Only the helm's marker asks the YOLO question, never its words (which can
/// quote a supervisor), so a refusal without the marker shows the same way even when its words speak of YOLO.
/// Specified: a refused Stop of a shows "Stop: <the helm's words>" on a's row and in a's menu, stays through an
/// unrelated pushed change, and goes once a's next Stop is sent; a Replace of b refused with the helm's YOLO words and
/// no marker shows them after "Replace: " and asks nothing.
#[gpui::test]
fn a_refusal_shows_on_the_row_and_in_its_menu_until_the_next_operation(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-stop");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Stop was sent");
    };
    fake.reply(id, 409, Some(json!("the agent is busy")));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some("Stop: the agent is busy")
    );
    test_support::click(cx, "session-menu-a");
    assert_eq!(
        label_of(cx, "session-menu-outcome-a").as_deref(),
        Some("Stop: the agent is busy")
    );
    test_support::keys(cx, "escape");
    push(
        &mut fake,
        &["a", "b"],
        &[(session("b").title("renamed"), 0)],
        cx,
    );
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some("Stop: the agent is busy")
    );

    choose(cx, "session-menu-a", "session-menu-stop");
    let [(again, _, _)] = pending(&mut fake)[..] else {
        panic!("a's next Stop was sent");
    };
    assert_eq!(
        label_of(cx, "session-outcome-a"),
        None,
        "the refusal goes when the next Stop starts, not when it settles"
    );

    let yolo = "buildbox asks before YOLO launches, and this launch is YOLO or uses the agent's own setting: nothing \
                was started.";
    choose(cx, "session-menu-b", "session-menu-replace");
    test_support::click(cx, "prompt-confirm");
    let [(replaced, _, _)] = pending(&mut fake)[..] else {
        panic!("b's Replace was sent");
    };
    fake.reply(again, 200, Some(json!({})));
    fake.reply(replaced, 409, Some(json!(yolo)));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-outcome-b").as_deref(),
        Some(format!("Replace: {yolo}").as_str())
    );
    assert!(
        !drawn(cx, "prompt-confirm"),
        "no YOLO question without the marker"
    );
}

/// Choose Stop from the menu of the row whose menu button is `button`, and answer the one Stop that sends with
/// `status` and `reply`.
fn stop_answered(
    fake: &mut FakeHelm,
    cx: &mut VisualTestContext,
    button: &'static str,
    status: u16,
    reply: Value,
) {
    choose(cx, button, "session-menu-stop");
    let [(id, _, _)] = pending(fake)[..] else {
        panic!("one Stop was sent from {button}");
    };
    fake.reply(id, status, Some(reply));
    test_support::settle(cx);
}

/// The operation store files a refusal under its own session, so one row's failure is never another row's, nor
/// cleared by another row's work. The old `terminal.spec.ts` case "a failed action's error is keyed to its own
/// session, not shared across rows" (reference commit `24bc0580`) pinned it; this guards the per-session key against
/// a refactor that shares one slot. Specified: a refused Stop of a shows "Stop: a is busy" on a's row and nothing on
/// b's; b's Stop succeeding leaves a's line and draws none on b; b's Stop refused shows "Stop: b is busy" on b's row
/// and leaves a's; a's next Stop, once sent, takes a's line away and leaves b's.
#[gpui::test]
fn a_refusal_stays_on_its_own_row_whatever_another_rows_stops_do(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    pending(&mut fake);
    let lines = |cx: &mut VisualTestContext| {
        (
            label_of(cx, "session-outcome-a"),
            label_of(cx, "session-outcome-b"),
        )
    };
    let a_busy = || Some("Stop: a is busy".to_owned());

    stop_answered(&mut fake, cx, "session-menu-a", 409, json!("a is busy"));
    assert_eq!(lines(cx), (a_busy(), None), "a's refusal is a's alone");
    stop_answered(&mut fake, cx, "session-menu-b", 200, json!({}));
    assert_eq!(lines(cx), (a_busy(), None), "b's success leaves a's");
    stop_answered(&mut fake, cx, "session-menu-b", 409, json!("b is busy"));
    let b_busy = Some("Stop: b is busy".to_owned());
    assert_eq!(lines(cx), (a_busy(), b_busy.clone()), "b's refusal is b's");

    choose(cx, "session-menu-a", "session-menu-stop");
    assert_eq!(
        pending(&mut fake).len(),
        1,
        "the premise: a's next Stop was sent"
    );
    assert_eq!(lines(cx), (None, b_busy), "a's next Stop clears a's alone");
}

/// SPEC.md, "Lifecycle operations": "A tab that cannot be closed does not prevent the other tabs from closing; the reply
/// names the failed tabs and the client shows that notice", which the old app put in the row's one slot as "Stop: …"
/// until the next operation. The completion notes it in the operation store's slot for the session, which must give it
/// up when any next operation of the session starts, not only a Stop. Specified: a Stop answered with a notice shows
/// "Stop: <notice>" on the row; it stays through a pushed change; a Rename of the session started through the operation
/// store takes it away, and it does not come back when that Rename succeeds.
#[gpui::test]
fn a_stops_notice_stays_on_its_row_until_the_next_operation_starts(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-stop");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Stop was sent");
    };
    let notice = "Terminal 2 could not be closed: host unavailable";
    fake.reply(id, 200, Some(json!({ "notice": notice })));
    test_support::settle(cx);
    let shown = format!("Stop: {notice}");
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some(shown.as_str())
    );
    push(&mut fake, &["a", "b"], &[(session("a").title("a"), 0)], cx);
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some(shown.as_str())
    );

    cx.update(|_, cx| {
        let operations = session::current_operations(cx).unwrap();
        let rename = super::rename::command("a", "renamed").unwrap();
        operations.update(cx, |operations, cx| {
            operations
                .start(&OpKey::session("a"), rename, cx, |_, _| {})
                .unwrap()
        });
    });
    test_support::settle(cx);
    assert_eq!(label_of(cx, "session-outcome-a"), None);
    let [rename] = &fake.commands()[..] else {
        panic!("one Rename was sent");
    };
    fake.reply(rename.id, 200, None);
    test_support::settle(cx);
    assert_eq!(label_of(cx, "session-outcome-a"), None);
}

/// a and b with a open, and a's Replace asked from its row, confirmed and sent: the window, the fake, and the Replace's
/// id for the test to answer.
fn replacing_the_open_session(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext, u64) {
    replacing_open(cx, session("a"))
}

/// [`replacing_the_open_session`] with `a` as the open session's fixture (its key must be `a`), so a test can choose
/// the Delete guard its Replace question freezes.
fn replacing_open(cx: &mut TestAppContext, a: Fixture) -> (FakeHelm, &mut VisualTestContext, u64) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(a, 0), (session("b"), 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    pending(&mut fake);
    choose(cx, "session-menu-a", "session-menu-replace");
    test_support::click(cx, "prompt-confirm");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Replace was sent");
    };
    (fake, cx, id)
}

/// The helm's reply to a's Replace once it has made n: n's own session, as the helm's fixture shapes one, and the notice
/// the source's Delete carried.
fn replacement_reply() -> Value {
    let mut reply = session("n").entry().value;
    reply["delete_notice"] = json!("the checkout moved to the trash");
    reply
}

/// What the helm does once a's Replace has made n: it pushes a gone and n listed, then answers with n's session and the
/// notice the source's Delete carried. Between the two, the selection must not have moved for the removal: `selected`
/// is what was open just before.
fn the_helm_replaces_a_with_n(
    fake: &mut FakeHelm,
    cx: &mut VisualTestContext,
    id: u64,
    selected_before: &str,
) {
    fake.push(change(
        listing_meta(&["n", "b"], 2, false),
        &[session("n")],
        &["a"],
    ));
    test_support::settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some(selected_before),
        "the source's removal moves no selection while its Replace runs"
    );
    fake.reply(id, 200, Some(replacement_reply()));
    test_support::settle(cx);
}

/// SPEC.md: "A completed Replace opens its replacement only if the source is still selected", and "If the user has not
/// moved on, the result may take focus" ("Who may move focus"): the user sat and waited for this one. The reply must
/// move focus itself: the replacement's terminal only becoming home would leave focus on whatever control held it. So a
/// control holds focus when the reply lands, with no input since Confirm: b's menu button, focused before the question
/// was asked (its menu opened and closed), which is where closing the question returns focus. The question is asked
/// through the list's own Replace, as the menu's item asks it, so that its opener is that button. The reply's
/// `delete_notice` is the source's Delete reporting, and the source's row is gone, so it shows in the list's Delete
/// notice band as an ordinary Delete's notice does, unprefixed, and not on the replacement's row (which once stood in
/// for the band). Specified: focus on b's button, a's Replace asked and confirmed with Enter, focus back on b's button;
/// the helm removes a and lists n; the reply opens n, whose terminal takes focus, the band shows the notice's words,
/// and n's row shows no line.
#[gpui::test]
fn a_replace_of_the_open_session_opens_its_replacement_and_focuses_it(cx: &mut TestAppContext) {
    replace_the_open_session_from_bs_button(cx, Confirm::Enter);
}

/// The same as [`a_replace_of_the_open_session_opens_its_replacement_and_focuses_it`] with Confirm clicked. The
/// question's card sits partly over the terminal, and a click on Confirm once also reached the terminal under it,
/// which took focus, so a click left focus on the terminal where Enter left it on the question's opener. Specified: the
/// same, with Confirm pressed by a mouse click.
#[gpui::test]
fn a_replace_confirmed_with_a_click_returns_focus_like_enter(cx: &mut TestAppContext) {
    replace_the_open_session_from_bs_button(cx, Confirm::Click);
}

/// How a test presses a question's Confirm.
#[derive(Clone, Copy)]
enum Confirm {
    Enter,
    Click,
}

/// The two focus tests above: see the first's docs.
fn replace_the_open_session_from_bs_button(cx: &mut TestAppContext, confirm: Confirm) {
    let (mut fake, _connection) = started(cx);
    fake.connect(snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    cx.run_until_parked();
    let (root, cx) = cx.add_window_view(|window, cx| {
        let main = cx.new(|cx| MainView::new(window, cx));
        Root::new(main, window, cx)
    });
    cx.update(|window, _| window.activate_window());
    test_support::settle(cx);
    let list = cx.update(|_, cx| {
        let main = root.read(cx).view().clone().downcast::<MainView>().unwrap();
        main.read(cx).session_list().clone()
    });
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    pending(&mut fake);
    test_support::click(cx, "session-menu-b");
    test_support::keys(cx, "escape");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-b"),
        "the premise: focus is on b's button"
    );

    cx.update(|window, cx| {
        let mark = Mark::take(cx);
        list.update(cx, |list, cx| list.replace("a", &mark, window, cx));
    });
    test_support::settle(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-confirm"),
        "the premise: the question starts on Confirm"
    );
    match confirm {
        Confirm::Enter => test_support::keys(cx, "enter"),
        Confirm::Click => test_support::click(cx, "prompt-confirm"),
    }
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Replace was sent");
    };
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-b"),
        "the premise: the question gave focus back to b's button, and nothing was touched since Confirm"
    );
    the_helm_replaces_a_with_n(&mut fake, cx, id, "a");
    assert_eq!(selected(cx).as_deref(), Some("n"));
    assert!(
        terminal_focused(cx),
        "the replacement's terminal took focus from b's button"
    );
    assert_eq!(
        label_of(cx, delete_notice::TEXT_ID).as_deref(),
        Some("the checkout moved to the trash")
    );
    assert_eq!(label_of(cx, "session-outcome-n"), None);
}

/// The hold on a Replace's source ends with the Replace however it ends. A Replace can be refused after the helm already
/// removed its source (the helm's "whether the original … was removed is unknown"), and then the list must move on as
/// for any removal rather than keep a selection that is gone. Those words belong to the source, whose row is gone, so
/// nothing shows them (recorded in the parity file's deviations). Specified: with a's Replace sent, a pushed removal of
/// a with n listed keeps a selected; a refusal then moves the selection off a, to n, and the refusal's words are
/// painted nowhere.
#[gpui::test]
fn a_replace_refused_after_its_source_went_lets_the_selection_move_on(cx: &mut TestAppContext) {
    let (mut fake, cx, id) = replacing_the_open_session(cx);
    fake.push(change(
        listing_meta(&["n", "b"], 2, false),
        &[session("n")],
        &["a"],
    ));
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is held");
    let words = "created replacement session n but whether the original a was removed is unknown after a lost reply; \
                 the replacement exists — check a before deleting it or retrying";
    fake.reply(id, 500, Some(json!(words)));
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("n"));
    assert!(
        !texts(cx)
            .iter()
            .any(|text| text.contains("whether the original")),
        "the source's refusal has no row to show on"
    );
}

/// The helm lists the replacement before it removes the source and answers, so the user can Stop it meanwhile. When the
/// notice went on the replacement's row it had to be dropped then (a note under that Stop would have outlived it), so
/// the one outcome SPEC.md says is never silent went unseen. The band belongs to no session's operations, so nothing
/// running anywhere keeps the notice out of it. Specified: with a's Replace sent and n listed without tabs, n's Stop is
/// sent; the Replace's reply carries a delete notice, which the band shows at once; once n's Stop succeeds, the band
/// still shows it and n's row shows no line.
#[gpui::test]
fn a_replaces_delete_notice_shows_in_the_band_while_the_replacement_has_work_running(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx, id) = replacing_the_open_session(cx);
    let mut listed = change(listing_meta(&["n", "b"], 2, false), &[], &["a"]);
    listed.upsert = vec![entry(&session("n"), 0)];
    fake.push(listed);
    test_support::settle(cx);
    choose(cx, "session-menu-n", "session-menu-stop");
    let [(stopped, _, _)] = pending(&mut fake)[..] else {
        panic!("n's Stop was sent");
    };
    fake.reply(id, 200, Some(replacement_reply()));
    test_support::settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some("n"),
        "the premise: the reply was read and opened n (a was open and held, so only a decoded reply moves it)"
    );
    assert_eq!(
        label_of(cx, delete_notice::TEXT_ID).as_deref(),
        Some("the checkout moved to the trash"),
        "shown while n's Stop runs"
    );
    fake.reply(stopped, 200, Some(json!({})));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, delete_notice::TEXT_ID).as_deref(),
        Some("the checkout moved to the trash")
    );
    assert_eq!(label_of(cx, "session-outcome-n"), None);
}

/// SPEC.md, "Who may move focus": "Once the user has moved on, the result moves no focus". The replacement still opens,
/// since its source is still the open session, but focus stays where the user put it. Focus is put on a control (b's
/// menu button) so the difference shows: a terminal that took focus away from a control would be the result moving it.
/// Specified: with a's Replace sent, the user opens b's menu and closes it with Escape, leaving focus on b's menu
/// button; the reply opens n, and focus is still on b's button.
#[gpui::test]
fn a_replace_after_the_user_moved_on_opens_its_replacement_without_focus(cx: &mut TestAppContext) {
    let (mut fake, cx, id) = replacing_the_open_session(cx);
    test_support::click(cx, "session-menu-b");
    test_support::keys(cx, "escape");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-b"),
        "the premise: focus is on b's button"
    );
    the_helm_replaces_a_with_n(&mut fake, cx, id, "a");
    assert_eq!(selected(cx).as_deref(), Some("n"));
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-b"));
}

/// SPEC.md: a completed Replace whose source is no longer selected "appears in the list and the user stays where they
/// are". Specified: with a's Replace sent, the user opens b; the pushed removal and the reply leave b open, with n
/// listed.
#[gpui::test]
fn a_replace_after_the_user_opened_another_session_leaves_them_there(cx: &mut TestAppContext) {
    let (mut fake, cx, id) = replacing_the_open_session(cx);
    test_support::click(cx, "session-row-b");
    the_helm_replaces_a_with_n(&mut fake, cx, id, "b");
    assert_eq!(selected(cx).as_deref(), Some("b"));
    assert_eq!(label_of(cx, "session-row-n").as_deref(), Some("n"));
}

// ---- The YOLO question a Replace meets -----------------------------------------------------------------------------

/// The bounds a fresh frame draws the element `id` at, by the frame's own record (rows' items carry an id but no debug
/// selector).
pub(crate) fn node_bounds(cx: &mut VisualTestContext, id: &str) -> gpui::Bounds<Pixels> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("{id} is drawn"))
        .bounds
}

/// SPEC.md, "Creation": the GUI answers the helm's YOLO refusal "with a prominent confirmation, shown with the control
/// or surface that started the launch", and "a plain Replace's question, which arrives after its reply, takes focus
/// only if the user has not moved on since answering Replace", on Cancel. The question's words and answers are the
/// launcher's and tested there; this pins where it shows and that the helm's command-line sentence does not. Specified:
/// with a's Replace confirmed from its row's menu and nothing touched since, a YOLO refusal draws the question inside
/// a's row, focus on Cancel; no "Replace: …" line shows on a's row, and nothing more is sent.
#[gpui::test]
fn a_yolo_refused_replace_asks_the_launchers_question_in_its_row(cx: &mut TestAppContext) {
    let (mut fake, cx, id) = replacing_the_open_session(cx);
    refuse_for_yolo(&mut fake, cx, id);
    assert!(
        drawn_inside(cx, "prompt-confirm", "session-item-a"),
        "the question is drawn in a's row"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));
    assert_eq!(
        label_of(cx, "session-outcome-a"),
        None,
        "the helm's sentence is not shown"
    );
    assert!(fake.commands().is_empty(), "nothing more was sent");
}

/// The launcher's YOLO question for a Replace (`launcher::entry::ask_yolo_for_replace`) has the sidebar pass the
/// source's host and its name as the rest of the app shows it (`hosts::identity::display_name`): the question names the
/// machine an agent would act on unchecked, so a wrong or generic name would leave the user unable to tell which host
/// they are agreeing for. The fixtures' snapshot lists no hosts, so the name is the one the helm sent with the row.
/// Specified: the question's line about asking names the row's host, `buildbox`, not "this host".
#[gpui::test]
fn the_question_names_the_sources_host(cx: &mut TestAppContext) {
    let (mut fake, cx, id) = replacing_the_open_session(cx);
    refuse_for_yolo(&mut fake, cx, id);
    assert_eq!(
        label_of(cx, "yolo-asks").as_deref(),
        Some("buildbox asks before every YOLO launch. Nothing has been started.")
    );
}

/// SPEC.md: the GUI "retries with the override only when the user confirms", and the retry is the same Replace (the
/// source and the Delete guard the user's Replace question froze, here the old app's "keeps its confirmed nothing-alive
/// answer" case). Exactly once: a second Replace would be refused for a source the first already removed, after
/// creating a second replacement. Its success follows Replace's own rule. Specified: on an ended a without tabs, the
/// one-off answer sends one Replace of a, the body pinned with `confirm_yolo` set and `only_if_nothing_alive` as its
/// question froze it; the question goes; the helm's replacement n opens and its terminal takes focus; nothing else is
/// sent.
#[gpui::test]
fn the_one_off_answer_replaces_again_with_consent_exactly_once(cx: &mut TestAppContext) {
    let (mut fake, cx, id) = replacing_open(cx, ended("a"));
    refuse_for_yolo(&mut fake, cx, id);
    test_support::click(cx, "prompt-confirm");
    let sent = pending(&mut fake);
    let [(again, _, _)] = sent[..] else {
        panic!("one Replace was sent again, not {}", sent.len());
    };
    assert_eq!(requests(sent), [consented("a", true, false)]);
    assert!(!drawn(cx, "prompt-confirm"), "the question went");
    the_helm_replaces_a_with_n(&mut fake, cx, again, "a");
    assert_eq!(selected(cx).as_deref(), Some("n"));
    assert!(
        terminal_focused(cx),
        "the replacement's terminal took focus"
    );
    assert_eq!(pending(&mut fake), [], "no second Replace");
}

/// While the permanent answer's host write is out (the launcher's to send and test), the question must stay up with
/// its answers frozen, and the session's own Replace must not race the write's retry with a second request. Specified:
/// after the permanent answer only one command is out; meanwhile the question is still drawn in a's row through a
/// pushed change of another row, and a's menu Replace is unavailable and sends nothing; once the write succeeds,
/// exactly one Replace of a with consent is sent.
#[gpui::test]
fn the_permanent_answer_writes_the_host_setting_then_replaces_with_consent(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx, id) = replacing_the_open_session(cx);
    refuse_for_yolo(&mut fake, cx, id);
    test_support::click(cx, "prompt-alternative-0");
    let sent = fake.commands();
    let [write] = &sent[..] else {
        panic!("only the host write is out, not {} commands", sent.len());
    };

    push(
        &mut fake,
        &["a", "b"],
        &[(session("b").title("renamed"), 0)],
        cx,
    );
    assert!(
        drawn_inside(cx, "prompt-alternative-0", "session-item-a"),
        "the question stays while its write is out"
    );
    choose(cx, "session-menu-a", "session-menu-replace");
    assert!(
        drawn(cx, "session-menu"),
        "a's Replace is unavailable meanwhile: the menu is still open"
    );
    assert!(fake.commands().is_empty(), "and nothing was sent");
    test_support::keys(cx, "escape");

    fake.reply(write.id, 200, Some(json!({})));
    test_support::settle(cx);
    assert_eq!(requests(pending(&mut fake)), [consented("a", false, false)]);
}

/// SPEC.md, "Lifecycle operations": "an open lifecycle confirmation or a pending request refuses another lifecycle
/// action", and the YOLO question is the Replace's confirmation. A Stop or Delete meanwhile would also take the session's
/// key, so the answer's Replace would be refused and only logged after the question closed. Specified: with a's YOLO
/// question open in its row, a's menu draws Rename, Clone, Replace with…, Replace, Stop and Delete unavailable (a click
/// on each leaves the menu open), and a click on each of the open session's header's Stop, Restart and its caret,
/// Replace and its caret, Clone and Delete (a is open) opens nothing; nothing is sent. Once the question is cancelled,
/// a's menu Stop sends a's Stop.
#[gpui::test]
fn an_open_yolo_question_refuses_the_sessions_other_lifecycle_actions(cx: &mut TestAppContext) {
    let (mut fake, cx, id) = replacing_the_open_session(cx);
    refuse_for_yolo(&mut fake, cx, id);
    assert!(
        cx.update(|_, cx| super::replace_yolo::open("a", cx)),
        "the premise: the question is open"
    );
    test_support::click(cx, "session-menu-a");
    for item in [
        "session-menu-rename",
        "session-menu-clone",
        "session-menu-replace-with",
        "session-menu-replace",
        "session-menu-stop",
        "session-menu-delete",
    ] {
        test_support::click(cx, item);
        assert!(
            drawn(cx, "session-menu"),
            "{item} is unavailable: the menu is still open"
        );
    }
    test_support::keys(cx, "escape");
    // a is the open session, running without tabs, so its header's Stop would stop it at once and its Delete would ask.
    for button in [
        "session-header-stop",
        "session-header-restart",
        "session-header-restart-options",
        "session-header-replace",
        "session-header-replace-options",
        "session-header-clone",
        "session-header-delete",
    ] {
        test_support::click(cx, button);
        assert!(!any_layer(cx), "{button} opens nothing");
    }
    assert!(fake.commands().is_empty(), "nothing was sent");

    test_support::click(cx, "prompt-cancel");
    choose(cx, "session-menu-a", "session-menu-stop");
    assert_eq!(requests(pending(&mut fake)), [stop("a", true, false)]);
}

/// The launcher's YOLO question for a Replace (`launcher::entry::ask_yolo_for_replace`) hands its retry the answer's
/// mark, for the sidebar to combine with its own operation's moved-on flag (`Settled::user_moved_on`). The permanent
/// answer's retry is sent from the host write's reply, so the operation store's own mark, taken then, misses a press
/// the user made while the write was out; only the answer's mark sees it. Without the combination the replacement would
/// pull focus from wherever the user went. Specified: after the permanent answer, the user opens b's menu and closes it
/// with Escape while the write is out, leaving focus on b's menu button; the write succeeds, the consented Replace is
/// sent with nothing touched since, and the helm's replacement n opens with focus still on b's button.
#[gpui::test]
fn a_press_during_the_host_write_keeps_the_replacement_from_taking_focus(cx: &mut TestAppContext) {
    let (mut fake, cx, id) = replacing_the_open_session(cx);
    refuse_for_yolo(&mut fake, cx, id);
    test_support::click(cx, "prompt-alternative-0");
    let sent = fake.commands();
    let [write] = &sent[..] else {
        panic!("only the host write is out, not {} commands", sent.len());
    };
    test_support::click(cx, "session-menu-b");
    test_support::keys(cx, "escape");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-b"),
        "the premise: focus is on b's button"
    );
    fake.reply(write.id, 200, Some(json!({})));
    test_support::settle(cx);
    let [(again, _, _)] = pending(&mut fake)[..] else {
        panic!("one Replace was sent again");
    };
    the_helm_replaces_a_with_n(&mut fake, cx, again, "a");
    assert_eq!(selected(cx).as_deref(), Some("n"));
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-b"));
}

/// The launcher's YOLO question for a Replace (`launcher::entry::ask_yolo_for_replace`) is to be dropped with the UI
/// session. A reset drops the host write's reply, so a question whose write was out would otherwise stay frozen for
/// good, and with it the guard that keeps the session's own Replace back. Specified: with the permanent answer's write
/// out, the channel breaks and reconnects with the same sessions; a's row draws no question, and a's menu Replace asks
/// its Replace question again.
#[gpui::test]
fn a_reset_drops_the_question(cx: &mut TestAppContext) {
    let (mut fake, cx, id) = replacing_the_open_session(cx);
    refuse_for_yolo(&mut fake, cx, id);
    test_support::click(cx, "prompt-alternative-0");
    assert!(
        cx.update(|_, cx| super::replace_yolo::open("a", cx)),
        "the premise: the question is open, its write out"
    );
    fake.break_channel();
    test_support::settle(cx);
    fake.connect(snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    test_support::settle(cx);
    pending(&mut fake);
    assert!(
        !drawn(cx, "prompt-alternative-0"),
        "the question went with the reset"
    );
    choose(cx, "session-menu-a", "session-menu-replace");
    assert!(
        drawn(cx, "prompt-confirm"),
        "a's Replace asks again after the reset"
    );
}

/// SPEC.md, "Opening and closing": "Closing something the user opened returns focus to where it was before it
/// opened", and Cancel starts nothing; and a closed question must give its row back the height it took, since nothing
/// shows there any more. The question arrived where the Replace question had left focus, a's menu button, so that is
/// where Cancel returns it. Specified: a's row is as tall after the refused Replace's question is cancelled as before
/// the Replace was asked; Cancel sent nothing, the question went, a's row shows no line, and focus is back on a's menu
/// button.
#[gpui::test]
fn cancel_drops_the_question_and_gives_focus_back_to_the_rows_button(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    pending(&mut fake);
    let before = node_bounds(cx, "session-item-a").size.height;
    choose(cx, "session-menu-a", "session-menu-replace");
    test_support::click(cx, "prompt-confirm");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Replace was sent");
    };
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-a"),
        "the premise: the Replace question gave focus back to a's button"
    );
    refuse_for_yolo(&mut fake, cx, id);
    assert!(
        node_bounds(cx, "session-item-a").size.height > before,
        "the premise: the question made a's row taller"
    );
    test_support::click(cx, "prompt-cancel");
    assert!(!drawn(cx, "prompt-confirm"), "the question went");
    assert!(fake.commands().is_empty(), "Cancel sent nothing");
    assert_eq!(label_of(cx, "session-outcome-a"), None);
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-a"));
    assert_eq!(
        node_bounds(cx, "session-item-a").size.height,
        before,
        "a's row is as tall as before"
    );
}

/// The old app's "row Replace YOLO arrival respects focus" case: a user who went on to something else while the
/// Replace was out must not have focus pulled into the question, which waits where it belongs. Specified: with a's
/// Replace out, the user opens b's menu and closes it with Escape, leaving focus on b's menu button; the YOLO refusal
/// draws the question in a's row, and focus is still on b's button.
#[gpui::test]
fn a_yolo_question_after_the_user_moved_on_waits_without_focus(cx: &mut TestAppContext) {
    let (mut fake, cx, id) = replacing_the_open_session(cx);
    test_support::click(cx, "session-menu-b");
    test_support::keys(cx, "escape");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-b"),
        "the premise: focus is on b's button"
    );
    refuse_for_yolo(&mut fake, cx, id);
    assert!(drawn_inside(cx, "prompt-confirm", "session-item-a"));
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-b"));
}

/// SPEC.md: the confirmation is "scrolled into view". A wheel scroll is not moving on (only key presses and clicks
/// are), so a user who scrolled the list while the Replace was out still gets the question with focus, and a focused
/// control the list scrolled away would leave Cancel off screen. Specified: with 200 idle sessions, s000's Replace from
/// its row's menu is out while the list is scrolled far down (s001 no longer drawn); the YOLO refusal moves focus to
/// the question's Cancel, which lies wholly inside the window.
#[gpui::test]
fn a_yolo_question_that_takes_focus_scrolls_its_row_into_view(cx: &mut TestAppContext) {
    let rows: Vec<_> = (0..200)
        .map(|index| {
            // The fixture keeps a `&'static str` key, as every fixed key in these tests is; the test leaks a few.
            let key: &'static str = Box::leak(format!("s{index:03}").into_boxed_str());
            session(key).status(json!({"state": "idle"}))
        })
        .collect();
    let keys: Vec<&str> = rows.iter().map(|row| row.key).collect();
    let (mut fake, cx) = open_main(
        cx,
        vec![
            super::tests::listing(&keys, &rows),
            super::tests::preferences(&["s001"], None),
        ],
    );
    pending(&mut fake);
    choose(cx, "session-menu-s000", "session-menu-replace");
    test_support::click(cx, "prompt-confirm");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Replace was sent");
    };
    scroll_list(cx, px(5_000.));
    assert!(
        !frame(cx)
            .iter()
            .any(|node| node.element_id.as_deref() == Some("session-row-s001")),
        "the premise: the rows around s000 are scrolled away"
    );

    refuse_for_yolo(&mut fake, cx, id);
    test_support::settle(cx);
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));
    let view = cx.update(|window, _| window.viewport_size().height);
    let cancel = test_support::bounds(cx, "prompt-cancel");
    assert!(
        cancel.top() >= px(0.) && cancel.bottom() <= view,
        "Cancel is in view: {cancel:?} in a window {view:?} high"
    );
}

/// The scroll into view takes two frames because the first judges the row by the height it had without the question:
/// a row wholly in view then is left where it is, and only the next frame, which measures the row with the question
/// under it, sees the question's foot past the list's bottom edge. Specified: with 60 idle sessions and s001 open, the
/// Replace of the lowest row wholly inside the list's view is refused for YOLO; focus is on the question's Cancel, and
/// Cancel lies wholly inside the list's view.
#[gpui::test]
fn a_yolo_question_below_its_rows_last_line_is_scrolled_into_view_too(cx: &mut TestAppContext) {
    let rows: Vec<_> = (0..60)
        .map(|index| {
            // The fixture keeps a `&'static str` key, as every fixed key in these tests is; the test leaks a few.
            let key: &'static str = Box::leak(format!("s{index:03}").into_boxed_str());
            session(key).status(json!({"state": "idle"}))
        })
        .collect();
    let keys: Vec<&str> = rows.iter().map(|row| row.key).collect();
    let (mut fake, cx) = open_main(
        cx,
        vec![
            super::tests::listing(&keys, &rows),
            super::tests::preferences(&["s001"], None),
        ],
    );
    pending(&mut fake);
    let list = test_support::bounds(cx, "session-rows");
    // The lowest row the list's view holds whole: its question's foot can only end below the view.
    let lowest = frame(cx)
        .into_iter()
        .filter_map(|node| {
            let key = node
                .element_id
                .as_deref()?
                .strip_prefix("session-item-")?
                .to_owned();
            (node.bounds.top() >= list.top() && node.bounds.bottom() <= list.bottom())
                .then_some((node.bounds.bottom(), key))
        })
        .max_by(|a, b| f32::from(a.0).total_cmp(&f32::from(b.0)))
        .map(|(_, key)| key)
        .expect("the premise: some row is wholly in view");
    assert_ne!(
        lowest, "s059",
        "the premise: the list has rows below the view"
    );
    let button: &'static str = Box::leak(format!("session-menu-{lowest}").into_boxed_str());
    choose(cx, button, "session-menu-replace");
    test_support::click(cx, "prompt-confirm");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Replace was sent");
    };

    refuse_for_yolo(&mut fake, cx, id);
    test_support::settle(cx);
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));
    let list = test_support::bounds(cx, "session-rows");
    let cancel = test_support::bounds(cx, "prompt-cancel");
    assert!(
        cancel.top() >= list.top() && cancel.bottom() <= list.bottom(),
        "Cancel is in the list's view: {cancel:?} in {list:?}"
    );
}

/// Scroll the list down by `by` with the pointer over it, as a wheel or trackpad would (the list tests' helper).
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

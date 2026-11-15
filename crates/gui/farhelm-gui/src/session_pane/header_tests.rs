//! Tests of the open session's top row: the actions' order, labels and hover help, Restart's offer and its request,
//! that Stop, Replace and Delete from the header ask the row's questions and send the row's preconditions, a pending
//! Delete and Stop, exclusion, a session the list does not show, where Escape and Cancel leave focus, and the row's
//! layout at the 650-pixel floor with the real text system.
//!
//! Everything but the layout test runs in the real main view over the helm client's fake, as the session list's own
//! lifecycle tests do (whose fixtures these share), and reads what the window sent from the fake and what it drew from
//! GPUI's driver record of the frame. The header's buttons are `session-header-stop`, `-restart`, `-replace` and
//! `-delete`; its lines are `session-header-<restart|replace>-progress-label-<key>` (a long wait) and
//! `session-header-outcome-<key>` (a refusal or a notice). The first session of every snapshot is the open one.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, KeyedEntry, PREFERENCES_KEY, Preferences,
};
use gpui::{
    AppContext as _, Context, Entity, HeadlessAppContext, IntoElement, Modifiers,
    ParentElement as _, Render, Styled as _, TestAppContext, VisualTestContext, Window, div, px,
    size,
};
use gpui_component::Root;
use serde_json::{Value, json};

use super::SessionPane;
use super::header::{Action, TABS_SLOT, unlisted_help};
use crate::actions::{DELETE_SESSION_CHORD, DELETE_SESSION_SHORTCUT_NAME};
use crate::focus::Mark;
use crate::main_view::MainView;
use crate::session_list::SessionList;
use crate::session_list::lifecycle_tests::replace;
use crate::session_list::menu_tests::{any_layer, drawn, ended, entry, label_of};
use crate::session_list::outcome::PROGRESS_SHOWS_AFTER;
use crate::session_list::tests::{
    Fixture, change, listing_meta, open_main, painted, selected, session,
};
use crate::test_support::{self, focused_id, frame, meta, started};
use crate::{assets, bootstrap, focus, layers, root};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// Whether `command` is one of the lifecycle requests the header sends: a Stop, a Restart, a Replace or a Delete.
fn is_lifecycle(command: &FakeCommand) -> bool {
    command.method == Method::Delete
        || (command.method == Method::Post
            && ["/stop", "/restart", "/replace"]
                .iter()
                .any(|end| command.path.ends_with(end)))
}

/// The lifecycle requests the window sent since the last call, as (id, path, body), left unanswered for the test to
/// answer. Every other command (a preference write, a seen mark) is answered with success at once; the fake answers in
/// order, so this works only while nothing sent before it waits.
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

/// [`pending`]'s requests, each answered as a success with an empty reply, as (path, body): for tests about what is
/// sent, so that nothing waits behind a request they do not answer.
fn answered(fake: &mut FakeHelm) -> Vec<(String, Value)> {
    pending(fake)
        .into_iter()
        .map(|(id, path, body)| {
            fake.reply(id, 200, Some(json!({})));
            (path, body)
        })
        .collect()
}

/// `fixture`'s entry with `tabs` terminal tabs, whose restart offer is `offer` for an agent of kind `agent`.
fn offering(fixture: &Fixture, tabs: usize, offer: &str, agent: &str) -> KeyedEntry {
    let mut entry = entry(fixture, tabs);
    entry.value["restart_offer"] = json!(offer);
    entry.value["agent_kind"] = json!(agent);
    entry
}

/// The preferences entry, with the first of `order` the last visited (so it is open at startup) and Skip session
/// confirmations as `skip` says.
fn preferences_entry(order: &[&str], skip: bool) -> KeyedEntry {
    let preferences = Preferences {
        visit_history: order.iter().take(1).map(|key| key.to_string()).collect(),
        skip_session_confirmations: Some(skip),
        ..Preferences::default()
    };
    KeyedEntry {
        key: PREFERENCES_KEY.into(),
        value: serde_json::to_value(preferences).unwrap(),
    }
}

/// A snapshot of `entries` in that order, the first one open, and Skip session confirmations as `skip` says. The
/// session list's own `snapshot` takes fixtures; this one takes entries, so a test can set a row's restart offer.
fn snapshot_of(entries: Vec<KeyedEntry>, skip: bool) -> Vec<CollectionState> {
    let order: Vec<String> = entries.iter().map(|entry| entry.key.clone()).collect();
    let order: Vec<&str> = order.iter().map(String::as_str).collect();
    vec![
        CollectionState {
            collection: Collection::Sessions,
            meta: listing_meta(&order, order.len() as u64, false),
            entries,
        },
        CollectionState {
            collection: Collection::Preferences,
            meta: meta(&[PREFERENCES_KEY]),
            entries: vec![preferences_entry(&order, skip)],
        },
    ]
}

/// Push the preferences with Skip session confirmations as `skip`, `order`'s first session the last visited.
fn push_skip(fake: &mut FakeHelm, order: &[&str], skip: bool, cx: &mut VisualTestContext) {
    fake.push(CollectionChanges {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        upsert: vec![preferences_entry(order, skip)],
        remove: Vec::new(),
    });
    test_support::settle(cx);
}

/// The hover help shown once the pointer has rested on `selector` for the hover delay.
fn hover(cx: &mut VisualTestContext, selector: &'static str) -> Option<String> {
    let at = test_support::center(cx, selector);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    test_support::advance(cx, layers::HOVER_DELAY);
    cx.update(|window, cx| layers::shown_tooltip(window, cx))
        .map(|shown| shown.text.to_string())
}

/// Every line of text painted in a fresh frame.
fn texts(cx: &mut VisualTestContext) -> Vec<String> {
    painted(&frame(cx))
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

/// A plain Restart of `key` with this consent to stop a working agent, as the window sends it: the shared client body
/// with only the consent set (the old app sent the consent alone; the helm reads the unset fields as absent).
fn restart(key: &str, stop_if_running: bool) -> (String, Value) {
    (
        format!("/api/sessions/{key}/restart"),
        json!({
            "conversation": null,
            "stop_if_running": stop_if_running,
            "with": null,
            "with_command": null,
            "confirm_yolo": false,
        }),
    )
}

/// `key` as an idle session: its agent waits for input, which Restart stops without asking.
fn idle(key: &'static str) -> Fixture {
    session(key).status(json!({"state": "idle"}))
}

// ---- The row and its actions ---------------------------------------------------------------------------------------

/// SPEC.md, "Terminal experience": the top row holds, on the right, Stop, Restart, Replace (then Clone, the launcher's)
/// and Delete, with the terminal area's tabs on the left. Restart is named for its offer, as the old button's
/// `aria-label` was, because "Restart" alone does not say whether the conversation survives. Specified: the four
/// buttons are drawn left to right in that order, after the empty tabs region; they paint "Stop", "Restart", "Replace"
/// and "Delete" and are named "Stop", "Resume conversation", "Replace" and "Delete"; nothing is labelled Clone.
#[gpui::test]
fn the_row_draws_stop_restart_replace_and_delete_in_the_old_order(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, snapshot_of(vec![entry(&session("a"), 0)], false));
    let nodes = frame(cx);
    let at = |id: &str| {
        nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("{id} was drawn"))
    };
    let lefts: Vec<f32> = Action::ALL
        .iter()
        .map(|action| f32::from(at(action.id()).bounds.origin.x))
        .collect();
    assert!(
        lefts.windows(2).all(|pair| pair[0] < pair[1]),
        "left to right: {lefts:?}"
    );
    assert!(
        at(TABS_SLOT).bounds.origin.x < at(Action::Stop.id()).bounds.origin.x,
        "the tabs' region comes first"
    );
    let names: Vec<_> = Action::ALL
        .iter()
        .map(|action| at(action.id()).label.as_deref().map(str::to_owned))
        .collect();
    assert_eq!(
        names,
        ["Stop", "Resume conversation", "Replace", "Delete"].map(|name| Some(name.to_owned()))
    );
    let painted = painted(&nodes);
    for label in ["Stop", "Restart", "Replace", "Delete"] {
        assert!(
            painted.iter().any(|text| text == label),
            "{label} is painted"
        );
    }
    assert!(!painted.iter().any(|text| text == "Clone"), "no Clone yet");
}

/// Every control's hover help is the old header's tooltip, the words that tell a mouse user what each one does before
/// the click; Delete's names the platform's shortcut and drops "after you confirm" when Skip session confirmations is
/// on, so the help never promises a question that will not come. Specified: Stop's, Restart's (for a resumable
/// session), Replace's and Delete's help on a running session; on Linux the shortcut is spelled Ctrl+Shift+Backspace;
/// once the preference is pushed on, Delete's help has no "after you confirm".
#[gpui::test]
fn each_actions_hover_help_is_the_old_headers_tooltip(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&session("a"), 0)], false));
    pending(&mut fake);
    assert_eq!(
        hover(cx, "session-header-stop").as_deref(),
        Some("Stop the agent and close its terminal tabs")
    );
    assert_eq!(
        hover(cx, "session-header-restart").as_deref(),
        Some("Resume conversation — restarting resumes this session's own conversation.")
    );
    assert_eq!(
        hover(cx, "session-header-replace").as_deref(),
        Some("Replace: swap this session for a fresh conversation with the same settings")
    );
    assert_eq!(
        hover(cx, "session-header-delete"),
        Some(format!(
            "Delete: remove this session and its state, after you confirm ({DELETE_SESSION_SHORTCUT_NAME})"
        ))
    );
    #[cfg(not(target_os = "macos"))]
    assert_eq!(DELETE_SESSION_SHORTCUT_NAME, "Ctrl+Shift+Backspace");
    #[cfg(target_os = "macos")]
    assert_eq!(DELETE_SESSION_SHORTCUT_NAME, "⌘W");

    push_skip(&mut fake, &["a"], true, cx);
    // Off the button and back, so its tooltip is shown afresh with the help drawn now.
    let away = test_support::center(cx, TABS_SLOT);
    cx.simulate_mouse_move(away, None, Modifiers::none());
    test_support::settle(cx);
    assert_eq!(
        hover(cx, "session-header-delete"),
        Some(format!(
            "Delete: remove this session and its state ({DELETE_SESSION_SHORTCUT_NAME})"
        ))
    );
}

// ---- Restart -------------------------------------------------------------------------------------------------------

/// SPEC.md: Restart is "greyed out when the session cannot resume its conversation; their tooltips and accessible
/// descriptions explain the specific reason", and greyed rather than disabled, so the reason stays reachable. A click
/// must send nothing: a Restart that silently loses the conversation is a bug. Specified, one session open at a time:
/// for an uncaptured running Codex session, an agent without conversation reporting, a command launch without Resume
/// and an uncaptured exited Claude session, Restart is named "Restart unavailable", its hover help is the reason worded
/// as the old tooltip, and a click opens nothing and sends nothing.
#[gpui::test]
fn restart_says_why_it_is_unavailable_and_sends_nothing(cx: &mut TestAppContext) {
    let rows = vec![
        offering(&session("a"), 0, "not_captured", "codex"),
        offering(&session("b"), 0, "no_conversation_reporting", "generic"),
        offering(&session("c"), 0, "no_resume_command", "claude"),
        offering(&ended("d"), 0, "not_captured", "claude"),
    ];
    let (mut fake, cx) = open_main(cx, snapshot_of(rows, false));
    pending(&mut fake);
    for (key, row, help) in [
        (
            "a",
            "session-row-a",
            "Restart unavailable — for Codex sessions, Restart becomes available once you submit your first prompt; \
             until then, Replace starts the session over. (If Restart doesn't become available, please send feedback \
             from the Help (?) menu.)",
        ),
        (
            "b",
            "session-row-b",
            "Restart unavailable — Farhelm has no conversation reporting for this session's agent, so Replace starts \
             it over.",
        ),
        (
            "c",
            "session-row-c",
            "Restart unavailable — this command launch did not opt into Resume, so it has no resume command and \
             Replace starts it over.",
        ),
        (
            "d",
            "session-row-d",
            "Restart unavailable — Farhelm never captured this Claude session's conversation, so Restart can't \
             resume it; Replace starts the session over. (If you expected Restart here, please send feedback from the \
             Help (?) menu.)",
        ),
    ] {
        test_support::click(cx, row);
        pending(&mut fake);
        assert_eq!(
            selected(cx).as_deref(),
            Some(key),
            "the premise: {key} is open"
        );
        assert_eq!(
            label_of(cx, "session-header-restart").as_deref(),
            Some("Restart unavailable"),
            "{key}"
        );
        assert_eq!(
            hover(cx, "session-header-restart").as_deref(),
            Some(help),
            "{key}"
        );
        test_support::click(cx, "session-header-restart");
        assert!(!any_layer(cx), "{key}: nothing asks");
        assert_eq!(pending(&mut fake), [], "{key}: nothing is sent");
    }
}

/// SPEC.md: "Restart on a session whose agent is working … confirms"; an idle, waiting or unknown agent "is stopped and
/// relaunched without asking", and the request carries the consent the user gave, which the supervisor checks against
/// its own reading. Specified: on an idle session, Restart asks nothing and sends `stop_if_running: false`; on a working
/// one it asks "Still running — restarting stops the agent and its whole process tree first:" with focus on Confirm
/// restart and Cancel's help "Cancel: leave the agent running", Cancel sends nothing, and Confirm sends
/// `stop_if_running: true`.
#[gpui::test]
fn restart_asks_only_while_the_agent_works_and_sends_the_consent_it_was_given(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(
        cx,
        snapshot_of(vec![entry(&idle("a"), 0), entry(&session("b"), 0)], false),
    );
    pending(&mut fake);
    test_support::click(cx, "session-header-restart");
    assert!(!any_layer(cx), "an idle agent restarts unasked");
    assert_eq!(answered(&mut fake), [restart("a", false)]);

    test_support::click(cx, "session-row-b");
    pending(&mut fake);
    test_support::click(cx, "session-header-restart");
    assert!(drawn(cx, "prompt"), "a working agent asks");
    assert!(texts(cx).iter().any(|text| text
        == "Still running — restarting stops the agent and its whole process tree first:"));
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    assert_eq!(
        hover(cx, "prompt-cancel").as_deref(),
        Some("Cancel: leave the agent running")
    );
    test_support::click(cx, "prompt-cancel");
    assert_eq!(pending(&mut fake), [], "Cancel sends nothing");
    test_support::click(cx, "session-header-restart");
    test_support::click(cx, "prompt-confirm");
    assert_eq!(answered(&mut fake), [restart("b", true)]);
}

/// SPEC.md: Skip session confirmations skips the "header Restart" question, and the click then authorizes only the
/// state the screen showed: a working agent's Restart goes ahead and carries the consent to stop it. Specified: with the
/// preference on, Restart on a working session opens nothing and sends `stop_if_running: true`.
#[gpui::test]
fn a_skipped_restart_question_goes_ahead_with_consent_for_the_working_agent(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&session("a"), 0)], true));
    pending(&mut fake);
    test_support::click(cx, "session-header-restart");
    assert!(!any_layer(cx));
    assert_eq!(answered(&mut fake), [restart("a", true)]);
}

/// SPEC.md: "After a Restart request has waited about two seconds, an adjacent status shows a spinner, the host and
/// elapsed seconds, updated about once a second until the reply arrives", and a refusal shows as "Restart: <words>"
/// where the session is shown. Time is the executor's clock. Specified: with an idle session's Restart sent and
/// unanswered, nothing shows 1.9 seconds in; at two seconds the header says "Restarting on buildbox…" and "2 s", then
/// "3 s"; once the helm refuses, the line is gone, and the header and the row both say "Restart: the host refused this
/// operation".
#[gpui::test]
fn a_restart_says_after_two_seconds_how_long_it_has_waited_and_then_its_refusal(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&idle("a"), 0)], false));
    pending(&mut fake);
    test_support::click(cx, "session-header-restart");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Restart was sent");
    };
    let progress =
        |cx: &mut VisualTestContext| label_of(cx, "session-header-restart-progress-label-a");
    test_support::advance(cx, PROGRESS_SHOWS_AFTER - Duration::from_millis(100));
    assert_eq!(progress(cx), None, "nothing before two seconds");
    test_support::advance(cx, Duration::from_millis(100));
    assert_eq!(progress(cx).as_deref(), Some("Restarting on buildbox…"));
    assert!(texts(cx).iter().any(|text| text == "2 s"));
    test_support::advance(cx, Duration::from_secs(1));
    assert!(texts(cx).iter().any(|text| text == "3 s"));

    fake.reply(id, 409, Some(json!("the host refused this operation")));
    test_support::settle(cx);
    assert_eq!(progress(cx), None, "the line goes with the reply");
    for line in ["session-header-outcome-a", "session-outcome-a"] {
        assert_eq!(
            label_of(cx, line).as_deref(),
            Some("Restart: the host refused this operation"),
            "{line}"
        );
    }
}

/// The role a fresh frame gives the element drawn under the id `id`, and whether one was drawn at all.
fn role_of(cx: &mut VisualTestContext, id: &str) -> Option<Option<gpui::Role>> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .map(|node| node.role)
}

/// A wait shown twice must be announced once: while the open session's Replace waits, its row and the header both show
/// the line, and the old row stayed silent then (`announce: !selected`), leaving the header to say it; a Restart's
/// line is the header's alone, and announced. Specified: with the open session's Replace waiting three seconds, the
/// header's `session-header-replace-progress-a` line is a status and the row's `session-replace-progress-a` line, also
/// drawn, has no role; with an idle session's Restart waiting three seconds, the header's
/// `session-header-restart-progress-a` line is a status.
#[gpui::test]
fn only_the_headers_copy_of_a_wait_line_is_announced(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&session("a"), 0)], false));
    pending(&mut fake);
    test_support::click(cx, "session-header-replace");
    test_support::click(cx, "prompt-confirm");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Replace was sent");
    };
    test_support::advance(cx, Duration::from_secs(3));
    assert_eq!(
        role_of(cx, "session-header-replace-progress-a"),
        Some(Some(gpui::Role::Status)),
        "the header announces the wait"
    );
    assert_eq!(
        role_of(cx, "session-replace-progress-a"),
        Some(None),
        "the open row's copy is drawn but stays silent"
    );
    fake.reply(id, 409, Some(json!("refused")));
    test_support::settle(cx);

    fake.push(change(listing_meta(&["a"], 1, false), &[idle("a")], &[]));
    test_support::settle(cx);
    pending(&mut fake);
    test_support::click(cx, "session-header-restart");
    assert_eq!(
        pending(&mut fake).len(),
        1,
        "the premise: the Restart was sent unasked"
    );
    test_support::advance(cx, Duration::from_secs(3));
    assert_eq!(
        role_of(cx, "session-header-restart-progress-a"),
        Some(Some(gpui::Role::Status)),
        "the header announces a Restart's wait"
    );
}

// ---- Stop, Replace and Delete: the row's questions and preconditions ---------------------------------------------

/// The header's Stop is the row's Stop (SPEC.md, "Lifecycle operations"): the same question, from the same row, and the
/// same frozen preconditions, so the two entry points can never warn about different things or authorize different
/// work. While it runs, the header says so on the button, and the session's other actions wait. Specified: on a running
/// session with one tab, the header's Stop asks "Stop the agent and close 1 terminal tab?" with the title quoted and
/// focus on Confirm stop; Confirm sends the Stop with tabs shown and a live agent; while it is unanswered, the Stop
/// button reads "Stopping…" and Replace opens nothing; once answered, Stop reads "Stop" again.
#[gpui::test]
fn the_headers_stop_asks_the_rows_question_and_says_stopping_while_it_runs(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&session("a"), 1)], false));
    pending(&mut fake);
    test_support::click(cx, "session-header-stop");
    assert!(
        texts(cx)
            .iter()
            .any(|text| text == "Stop the agent and close 1 terminal tab?")
    );
    assert_eq!(
        label_of(cx, "session-stop-title-a").as_deref(),
        Some("\"a\"")
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    test_support::click(cx, "prompt-confirm");
    let [(id, path, body)] = &pending(&mut fake)[..] else {
        panic!("one Stop was sent");
    };
    assert_eq!((path.clone(), body.clone()), stop("a", false, false));

    assert_eq!(
        label_of(cx, "session-header-stop").as_deref(),
        Some("Stopping…")
    );
    test_support::click(cx, "session-header-replace");
    assert!(!any_layer(cx), "Replace waits while the Stop runs");
    fake.reply(*id, 200, Some(json!({})));
    test_support::settle(cx);
    assert_eq!(label_of(cx, "session-header-stop").as_deref(), Some("Stop"));
}

/// The header's Replace is the row's Replace: the same consequence, and the source's Delete guard for what the
/// question showed. Specified: on an ended session with one tab, the header's Replace asks with the old consequence
/// (the tab clause, then the ended agent's) and Confirm replace; Confirm sends the Replace guarded by the ended agent.
#[gpui::test]
fn the_headers_replace_asks_the_rows_question_and_sends_its_guard(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&ended("a"), 1)], false));
    pending(&mut fake);
    test_support::click(cx, "session-header-replace");
    assert!(texts(cx).iter().any(|text| text
        == "1 terminal tab is still open, and replacing closes it with everything running in it; replacing discards \
            the conversation; a fresh session with the same settings takes its place:"));
    assert!(texts(cx).iter().any(|text| text == "Confirm replace"));
    test_support::click(cx, "prompt-confirm");
    assert_eq!(answered(&mut fake), [replace("a", false, true)]);
}

/// In the macOS window the top row is half of the title band, where text keeps its presses (SPEC.md, "Desktop window
/// chrome"): while a Delete is in flight, a press on the "Deleting…" progress that stands in for the actions must not
/// drag the window. Pressed through the real row, with the band's decisions recorded instead of performed. macOS only,
/// where the band exists.
#[cfg(target_os = "macos")]
#[gpui::test]
fn the_delete_progress_in_the_title_band_keeps_its_presses(cx: &mut TestAppContext) {
    use crate::desktop::title_band::{Decisions, PressAction, Row};
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&ended("a"), 0)], false));
    pending(&mut fake);
    test_support::click(cx, "session-header-delete");
    test_support::click(cx, "prompt-confirm");
    assert!(
        drawn(cx, "session-header-delete-progress"),
        "the premise: the progress shows"
    );
    let at = test_support::center(cx, "session-header-delete-progress");
    cx.update(|_, cx| cx.set_global(Decisions::default()));
    cx.simulate_event(gpui::MouseDownEvent {
        button: gpui::MouseButton::Left,
        position: at,
        modifiers: gpui::Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseMoveEvent {
        position: at + gpui::point(gpui::px(6.), gpui::px(0.)),
        pressed_button: Some(gpui::MouseButton::Left),
        modifiers: gpui::Modifiers::none(),
    });
    cx.simulate_event(gpui::MouseUpEvent {
        button: gpui::MouseButton::Left,
        position: at,
        modifiers: gpui::Modifiers::none(),
        click_count: 1,
    });
    test_support::settle(cx);
    let decided = cx.update(|_, cx| cx.default_global::<Decisions>().0.clone());
    assert_eq!(decided, [(Row::SessionTop, PressAction::None)]);
}

/// SPEC.md: the header's Delete "asks first in the same anchored way, even for a session that has ended, with the same
/// consequence text the row's delete prompt shows", while the row deletes such a session unasked; and from the moment
/// it is committed, "the open session's header shows the same in place of its actions", which come back with a refusal.
/// Specified: on an ended session with no tabs, the header's Delete asks ("Delete anyway:") and Confirm sends the Delete
/// guarded by nothing alive; while it is unanswered the header shows "Deleting…" and none of its actions; a refusal
/// brings the four actions back and the header shows the helm's words.
#[gpui::test]
fn the_headers_delete_asks_even_with_nothing_alive_and_its_progress_replaces_the_actions(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&ended("a"), 0)], false));
    pending(&mut fake);
    test_support::click(cx, "session-header-delete");
    assert!(
        drawn(cx, "prompt"),
        "the header asks where the row would not"
    );
    assert!(texts(cx).iter().any(|text| text == "Delete anyway:"));
    test_support::click(cx, "prompt-confirm");
    let [(id, path, _)] = &pending(&mut fake)[..] else {
        panic!("one Delete was sent");
    };
    assert_eq!(path, "/api/sessions/a?only_if_nothing_alive=true");

    assert!(drawn(cx, "session-header-delete-progress"));
    assert_eq!(
        label_of(cx, "session-header-delete-progress-a").as_deref(),
        Some("Deleting…")
    );
    for action in Action::ALL {
        assert!(
            cx.debug_bounds(action.id()).is_none(),
            "{action:?} gave way to the progress"
        );
    }
    fake.reply(*id, 409, Some(json!("the session's tabs did not close")));
    test_support::settle(cx);
    assert!(!drawn(cx, "session-header-delete-progress"));
    for action in Action::ALL {
        assert!(cx.debug_bounds(action.id()).is_some(), "{action:?} is back");
    }
    assert_eq!(
        label_of(cx, "session-header-outcome-a").as_deref(),
        Some("the session's tabs did not close")
    );
}

/// SPEC.md: "a pending request refuses another lifecycle action" on that session, whichever surface started it. A
/// Stop sent from the row must leave the header's actions inert. Specified: with a Stop of the open session sent from
/// its row's menu and unanswered, the header's Restart, Replace and Delete each open nothing and send nothing.
#[gpui::test]
fn a_request_from_the_row_leaves_the_headers_actions_unavailable(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&idle("a"), 0)], false));
    pending(&mut fake);
    test_support::click(cx, "session-menu-a");
    test_support::click(cx, "session-menu-stop");
    test_support::settle(cx);
    assert_eq!(
        pending(&mut fake).len(),
        1,
        "the premise: a's Stop is in flight"
    );
    for action in [Action::Restart, Action::Replace, Action::Delete] {
        test_support::click(cx, action.id());
        assert!(!any_layer(cx), "{action:?} asks nothing");
    }
    assert!(
        fake.commands().iter().all(|command| !is_lifecycle(command)),
        "nothing more was sent"
    );
}

/// The header acts on the open session's listed row; a session the list does not show (a filter, or the list's cap,
/// which proves nothing about it being gone) has no row to ask from, so its actions are drawn unavailable and say why.
/// Specified: with a open, a truncated listing pushed without a keeps a open; each action's hover help is its unlisted
/// help; a click on Replace or Delete opens nothing.
#[gpui::test]
fn an_unlisted_open_session_draws_its_actions_unavailable_and_says_why(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(
        cx,
        snapshot_of(
            vec![entry(&session("a"), 0), entry(&session("b"), 0)],
            false,
        ),
    );
    pending(&mut fake);
    fake.push(change(listing_meta(&["b"], 2, true), &[], &["a"]));
    test_support::settle(cx);
    assert_eq!(
        selected(cx).as_deref(),
        Some("a"),
        "the premise: a stays open"
    );
    for action in Action::ALL {
        assert_eq!(
            hover(cx, action.id()),
            Some(unlisted_help(action)),
            "{action:?}"
        );
    }
    for action in [Action::Replace, Action::Delete] {
        test_support::click(cx, action.id());
        assert!(!any_layer(cx), "{action:?} asks nothing");
    }
    assert_eq!(pending(&mut fake), []);
}

/// The old `session-view-claims.spec.ts`: another client deleting the open session while its header Restart question
/// is open, or while its Restart runs, must leave navigation working, and must not leave the next session's header
/// unavailable because of the gone session's question or request. Specified: with a's Restart question open, a complete
/// listing pushed without a closes the question and opens b, whose header's Replace then asks; and with b's Restart sent
/// and unanswered, a complete listing without b opens c, whose header's Replace asks too, while b's Restart is still in
/// flight.
#[gpui::test]
fn a_session_deleted_elsewhere_leaves_the_next_sessions_header_usable(cx: &mut TestAppContext) {
    let rows = vec![
        entry(&session("a"), 0),
        entry(&idle("b"), 0),
        // Created before b, so that b is the newest once a goes (no visit names either).
        entry(&session("c").created(50), 0),
    ];
    let (mut fake, cx) = open_main(cx, snapshot_of(rows, false));
    pending(&mut fake);
    test_support::click(cx, "session-header-restart");
    assert!(
        drawn(cx, "prompt"),
        "the premise: a's Restart question is open"
    );
    fake.push(change(listing_meta(&["b", "c"], 2, false), &[], &["a"]));
    test_support::settle(cx);
    assert!(!any_layer(cx), "the question went with its session");
    assert_eq!(selected(cx).as_deref(), Some("b"));
    pending(&mut fake);
    test_support::click(cx, "session-header-replace");
    assert!(drawn(cx, "prompt"), "b's header asks");
    test_support::click(cx, "prompt-cancel");

    test_support::click(cx, "session-header-restart");
    let [(id, path, _)] = &pending(&mut fake)[..] else {
        panic!("b's Restart was sent");
    };
    assert_eq!(path, "/api/sessions/b/restart");
    fake.push(change(listing_meta(&["c"], 1, false), &[], &["b"]));
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("c"));
    test_support::click(cx, "session-header-replace");
    assert!(
        drawn(cx, "prompt"),
        "c's header asks while b's Restart runs"
    );
    test_support::click(cx, "prompt-cancel");
    fake.reply(*id, 200, Some(json!({})));
    test_support::settle(cx);
}

// ---- Focus ---------------------------------------------------------------------------------------------------------

/// SPEC.md, "Keyboard focus": "Escape cancels the header's Stop, Restart, Replace or Delete question and returns focus
/// to the terminal", while "Closing something the user opened returns focus to where it was before it opened" for every
/// other close. The click rule is Linux's, so a click on a header button focuses it and the question's opener is that
/// button. Specified: Replace asked by a click and answered Cancel leaves focus on the Replace button; asked again and
/// dismissed with Escape, focus is on the terminal and nothing was sent.
#[gpui::test]
fn escape_returns_to_the_terminal_and_cancel_to_the_button(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&session("a"), 0)], false));
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    pending(&mut fake);
    test_support::click(cx, "session-header-replace");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("prompt-confirm"),
        "the premise: the question has focus"
    );
    test_support::click(cx, "prompt-cancel");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-header-replace"),
        "Cancel returns to the button"
    );

    test_support::click(cx, "session-header-replace");
    assert!(drawn(cx, "prompt"), "the premise: asked again");
    test_support::keys(cx, "escape");
    assert!(!any_layer(cx));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("terminal-grid"),
        "Escape returns to the terminal"
    );
    assert_eq!(pending(&mut fake), []);
}

/// The open question's button reads as pressed and does not answer or reopen it on a second click, as the row's menu
/// button is inert while its question is open: a second click must not stack a second question or answer the first.
/// GPUI's driver record does not carry the expanded state, so "pressed" is read where the button takes it from, the
/// question the header records as open. Specified: with the header's Replace question open, the header says Replace's
/// question is the open one; a click on the Replace button leaves the one question open and sends nothing; once it is
/// answered Cancel, no button reads as pressed.
#[gpui::test]
fn the_button_of_an_open_question_is_pressed_and_inert(cx: &mut TestAppContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(snapshot_of(vec![entry(&session("a"), 0)], false));
    cx.run_until_parked();
    let (main, cx) = test_support::open_view(cx, MainView::new);
    let pane = cx.update(|_, cx| main.read(cx).session_pane().clone());
    let pressed = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| pane.update(cx, |pane, cx| pane.header.open_question(window, cx)))
    };
    pending(&mut fake);
    test_support::click(cx, "session-header-replace");
    let questions = |cx: &mut VisualTestContext| {
        frame(cx)
            .iter()
            .filter(|node| node.element_id.as_deref() == Some("prompt"))
            .count()
    };
    assert_eq!(questions(cx), 1, "the premise: one question");
    assert_eq!(
        pressed(cx),
        Some(Action::Replace),
        "Replace reads as pressed"
    );
    test_support::click(cx, "session-header-replace");
    assert_eq!(questions(cx), 1, "still one question");
    assert_eq!(pending(&mut fake), []);
    test_support::click(cx, "prompt-cancel");
    assert_eq!(
        pressed(cx),
        None,
        "nothing reads as pressed once it is answered"
    );
}

/// SPEC.md, "Desktop window chrome": the Delete shortcut "invokes the open session's header Delete action with its
/// ordinary confirmation", and the header's Delete "asks first in the same anchored way, even for a session that has
/// ended". The row's Delete goes ahead unasked on a session with nothing alive, so this is where the two differ.
/// Specified: on an ended session with no tabs, the chord asks the Delete question below the header's Delete with
/// "Delete anyway:" and nothing sent; Enter sends the Delete guarded by nothing alive.
#[gpui::test]
fn the_delete_shortcut_asks_the_headers_question_even_with_nothing_alive(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&ended("a"), 0)], false));
    pending(&mut fake);
    test_support::keys(cx, DELETE_SESSION_CHORD);
    assert!(drawn(cx, "prompt"), "the shortcut asks");
    let button = test_support::bounds(cx, "session-header-delete");
    let question = test_support::bounds(cx, "prompt");
    assert!(
        question.top() >= button.bottom(),
        "asked below the header's Delete"
    );
    assert!(texts(cx).iter().any(|text| text == "Delete anyway:"));
    assert_eq!(pending(&mut fake), [], "nothing sent before the answer");
    test_support::keys(cx, "enter");
    let [(_, path, _)] = &pending(&mut fake)[..] else {
        panic!("one Delete was sent");
    };
    assert_eq!(path, "/api/sessions/a?only_if_nothing_alive=true");
}

// ---- Layout at the 650-pixel floor -----------------------------------------------------------------------------------

/// The node drawn for `action`'s button in `nodes`; panics, naming what was drawn, when there is none.
fn drawn_node(nodes: &[gpui::DriverNode], action: Action) -> &gpui::DriverNode {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(action.id()))
        .unwrap_or_else(|| {
            let ids: Vec<_> = nodes
                .iter()
                .filter_map(|node| node.element_id.clone())
                .collect();
            panic!("{action:?} was not drawn; drawn: {ids:?}")
        })
}

/// Each of the four actions in `nodes` is drawn whole (nothing of it clipped) inside a 650-pixel pane, and all four sit
/// on one line.
fn assert_whole_on_one_line(nodes: &[gpui::DriverNode]) {
    let mut tops = Vec::new();
    for action in Action::ALL {
        let node = drawn_node(nodes, action);
        let bounds = node.bounds;
        assert!(
            bounds.size.width > px(0.) && node.visible_bounds == bounds,
            "{action:?} is drawn whole: {bounds:?}, visible {:?}",
            node.visible_bounds
        );
        assert!(
            bounds.origin.x >= px(0.) && bounds.origin.x + bounds.size.width <= px(650.),
            "{action:?} is inside the pane: {bounds:?}"
        );
        tops.push(bounds.origin.y);
    }
    assert!(
        tops.windows(2).all(|pair| pair[0] == pair[1]),
        "one line: {tops:?}"
    );
}

/// A root view drawing the pane alone across the window, wired as every root view is. It also keeps a session list,
/// undrawn, because the list owns the selection rules: without one nothing opens a session, and the pane has no top row
/// to draw.
struct PaneOnly {
    pane: Entity<SessionPane>,
    _list: Entity<SessionList>,
    root: root::Attachment,
}

impl Render for PaneOnly {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        root::wrap(
            div().size_full().child(self.pane.clone()),
            &self.root,
            window,
            cx,
        )
    }
}

/// SPEC.md: "Text size, Stop, both parts of each split button, Clone, Delete and the tunnel pill remain fully visible
/// and keyboard reachable from a 650px main pane; … Narrower panes may clip the row, which never wraps." Text width
/// decides this, and the test context's text system gives every character the same width, so this runs with the real
/// text system and the bundled fonts (laid out, not rendered). Nothing else in the top row competes for its width yet
/// (the tab strip that will is the terminal area's), so what this pins today is that the four actions fit whole on one
/// line at 650 pixels; it becomes a real guard once the left region fills. The one thing that could already push them
/// is a long host name, which a Restart's wait line names: the line sits below the row, so it must not. Specified: in a
/// 650-pixel pane, with an idle session on a host with a long name, each of the four actions is drawn whole inside the
/// pane, all on one line, and each tracks a focus handle, so Tab reaches it; once its Restart has waited three seconds
/// (the wait line drawn), the four are still drawn whole on one line.
#[test]
fn every_action_is_whole_and_reachable_in_a_650_pixel_pane() {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        assets::UI_FONT,
    ));
    let mut cx =
        HeadlessAppContext::with_platform(text_system, Arc::new(assets::Assets), || Ok(None));
    cx.update(bootstrap::install).unwrap();
    let (mut fake, handle, events) = FakeHelm::new();
    let _connection = cx.update(|cx| crate::session::start(handle, events, None, cx));
    cx.run_until_parked();
    let mut long = entry(&idle("a"), 0);
    long.value["host_name"] = json!(format!(
        "a-very-long-host-name{}.example.internal",
        "-x".repeat(40)
    ));
    fake.connect(snapshot_of(vec![long], false));
    cx.run_until_parked();
    let pane = Rc::new(RefCell::new(None));
    let kept = pane.clone();
    let window = cx
        .open_window(size(px(650.), px(300.)), |window, cx| {
            let view = cx.new(|cx| {
                let pane = cx.new(|cx| SessionPane::new(window, cx));
                kept.replace(Some(pane.clone()));
                PaneOnly {
                    pane,
                    _list: cx.new(SessionList::new),
                    root: root::attach(window, cx),
                }
            });
            cx.new(|cx| Root::new(view, window, cx))
        })
        .unwrap();
    let pane = pane.borrow().clone().expect("the pane was built");
    let draw = |cx: &mut HeadlessAppContext| {
        for _ in 0..3 {
            cx.run_until_parked();
            cx.update_window(window.into(), |_, window, cx| {
                window.set_driver_enabled(true);
                window.draw(cx).clear(cx);
            })
            .unwrap();
        }
        cx.update_window(window.into(), |_, window, _| window.driver_nodes().to_vec())
            .unwrap()
    };
    // Before the Restart: every action enabled, so each tracks its focus handle and Tab reaches it.
    let idle_nodes = draw(&mut cx);
    assert_whole_on_one_line(&idle_nodes);
    for action in Action::ALL {
        let node = drawn_node(&idle_nodes, action);
        assert!(node.focus.is_some(), "{action:?} tracks a focus handle");
    }
    cx.update_window(window.into(), |_, window, cx| {
        let mark = Mark::take(cx);
        pane.update(cx, |pane, cx| {
            pane.header_action(Action::Restart, &mark, window, cx)
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.advance_clock(Duration::from_secs(3));
    // While the Restart waits: the actions are unavailable (so not Tab stops), and the line naming the long host is
    // drawn below the row without moving them.
    let waiting_nodes = draw(&mut cx);
    assert!(
        waiting_nodes
            .iter()
            .any(|node| node.element_id.as_deref()
                == Some("session-header-restart-progress-label-a")),
        "the premise: the Restart's wait line, naming the long host, is drawn"
    );
    assert_whole_on_one_line(&waiting_nodes);
}

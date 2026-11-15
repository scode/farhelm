//! Tests of the open session's top row: the actions' order, labels and hover help, Restart's offer and its request,
//! that Stop, Replace and Delete from the header ask the row's questions and send the row's preconditions, a pending
//! Delete and Stop, exclusion, where Escape and Cancel leave focus, the row's layout at the 650-pixel floor with the
//! real text system, and the terminal text size group at the start of the actions (its steps, readout, range, shortcut,
//! and the size a terminal opens at). The header of an open session the list does not show is `carried_tests.rs`'s.
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
use super::header::{Action, TABS_SLOT};
use crate::actions::{DELETE_SESSION_CHORD, DELETE_SESSION_SHORTCUT_NAME};
use crate::focus::Mark;
use crate::main_view::MainView;
use crate::session_list::SessionList;
use crate::session_list::lifecycle_tests::{
    consented, drawn_inside, node_bounds, refuse_for_yolo, replace,
};
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
        // A helm that has shown the Welcome card, which would otherwise open over the main view (`crate::welcome`).
        welcome_seen: Some(true),
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

/// An ended a without tabs, open, beside a running b, a's Replace sent from the header through its question and
/// unanswered: the window, the fake and the Replace's id. The ended agent makes the Replace's Delete guard "nothing
/// alive", so a test can tell that a retry carries the guard the Replace froze.
fn replacing_from_the_header(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext, u64) {
    let (mut fake, cx) = open_main(
        cx,
        snapshot_of(vec![entry(&ended("a"), 0), entry(&session("b"), 0)], false),
    );
    pending(&mut fake);
    test_support::click(cx, "session-header-replace");
    test_support::click(cx, "prompt-confirm");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Replace was sent");
    };
    (fake, cx, id)
}

/// The height between the bottom of the header's top row and the top of the terminal: what the lines below the row
/// take from the terminal.
fn lines_height(cx: &mut VisualTestContext) -> gpui::Pixels {
    let row = test_support::bounds(cx, "session-header").bottom();
    node_bounds(cx, "terminal-grid").top() - row
}

/// SPEC.md, "Creation": the YOLO confirmation is "shown with the control or surface that started the launch", so a
/// Replace from the header meets it in the header, not at a row the user did not use; it sits below the top row, as the
/// header's other lines do, so it cannot push an action out of the 650-pixel floor. Specified: with an ended a's
/// Replace confirmed from the header and nothing touched since, a YOLO refusal draws one question, below the header's
/// top row and not in a's list row, with focus on its Cancel; the one-off answer sends exactly one Replace of a, the
/// body pinned with `confirm_yolo` set beside the nothing-alive guard a's question froze.
#[gpui::test]
fn a_yolo_refused_replace_from_the_header_asks_below_its_top_row(cx: &mut TestAppContext) {
    let (mut fake, cx, id) = replacing_from_the_header(cx);
    refuse_for_yolo(&mut fake, cx, id);
    let questions = frame(cx)
        .iter()
        .filter(|node| node.element_id.as_deref() == Some("prompt-confirm"))
        .count();
    assert_eq!(questions, 1, "one question");
    assert!(
        !drawn_inside(cx, "prompt-confirm", "session-item-a"),
        "not in a's row"
    );
    assert!(
        test_support::bounds(cx, "prompt-confirm").top()
            >= test_support::bounds(cx, "session-header").bottom(),
        "below the top row"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));
    test_support::click(cx, "prompt-confirm");
    assert_eq!(answered(&mut fake), [consented("a", true, false)]);
    assert_eq!(pending(&mut fake), [], "exactly once");
}

/// SPEC.md, "Terminal experience": "The terminal loses no height in the steady state". The question takes height from
/// the terminal while it is up, and a cancelled one shows nothing, so the terminal must get all of it back; an empty
/// band left behind would stay until the session's next Replace. Specified: the distance from the top row to the
/// terminal is the same after the header's YOLO question is cancelled as before the Replace was asked, and larger while
/// the question is up.
#[gpui::test]
fn cancelling_the_headers_question_gives_the_terminal_its_height_back(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(
        cx,
        snapshot_of(vec![entry(&ended("a"), 0), entry(&session("b"), 0)], false),
    );
    pending(&mut fake);
    let before = lines_height(cx);
    test_support::click(cx, "session-header-replace");
    test_support::click(cx, "prompt-confirm");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Replace was sent");
    };
    refuse_for_yolo(&mut fake, cx, id);
    assert!(
        lines_height(cx) > before,
        "the premise: the question takes height"
    );
    test_support::click(cx, "prompt-cancel");
    assert!(
        !drawn(cx, "prompt-confirm"),
        "the premise: the question went"
    );
    assert_eq!(lines_height(cx), before);
}

/// The header is drawn only for the open session, so a question asked from it would be shown nowhere once the user
/// opened another session, and its refusal's record is gone (the question says it instead). SPEC.md, "Lifecycle
/// operations": "A failure or partial result appears on the source session's row and at the top when it is opened
/// again". Specified: with a's Replace from the header out, the user opens b; the YOLO refusal draws the question in
/// a's row and not in b's header; once the user opens a again, it is drawn below the header's top row and not in a's
/// row, and there is still one question.
#[gpui::test]
fn a_header_question_shows_at_its_row_while_its_session_is_not_open(cx: &mut TestAppContext) {
    let (mut fake, cx, id) = replacing_from_the_header(cx);
    test_support::click(cx, "session-row-b");
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    refuse_for_yolo(&mut fake, cx, id);
    assert!(
        drawn_inside(cx, "prompt-confirm", "session-item-a"),
        "the question is at a's row while b is open"
    );
    let questions = |cx: &mut VisualTestContext| {
        frame(cx)
            .iter()
            .filter(|node| node.element_id.as_deref() == Some("prompt-confirm"))
            .count()
    };
    assert_eq!(questions(cx), 1, "one question, so none in b's header");
    test_support::click(cx, "session-row-a");
    assert!(
        !drawn_inside(cx, "prompt-confirm", "session-item-a"),
        "not at a's row once a is open"
    );
    assert!(
        test_support::bounds(cx, "prompt-confirm").top()
            >= test_support::bounds(cx, "session-header").bottom(),
        "below the top row"
    );
    assert_eq!(questions(cx), 1, "one question");
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
/// on one line; so are the text size group's two buttons and readout, which share that line and whose buttons track a
/// focus handle, so Tab reaches them (SPEC.md names text size first among what must stay whole and reachable at 650
/// pixels).
fn assert_whole_on_one_line(nodes: &[gpui::DriverNode]) {
    use crate::terminal::text_size::{LARGER_ID, READOUT_ID, SMALLER_ID};
    let action_row = drawn_node(nodes, Action::Stop).bounds;
    let middle = action_row.origin.y + action_row.size.height / 2.;
    for id in [SMALLER_ID, READOUT_ID, LARGER_ID] {
        let node = nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("{id} was drawn"));
        let bounds = node.bounds;
        assert!(
            bounds.size.width > px(0.)
                && node.visible_bounds == bounds
                && bounds.origin.x >= px(0.)
                && bounds.origin.x + bounds.size.width <= px(650.),
            "{id} is drawn whole inside the pane: {bounds:?}, visible {:?}",
            node.visible_bounds
        );
        assert!(
            bounds.origin.y <= middle && middle <= bounds.origin.y + bounds.size.height,
            "{id} is on the actions' line: {bounds:?}, the line's middle at {middle:?}"
        );
        if id != READOUT_ID {
            assert!(node.focus.is_some(), "{id} tracks a focus handle");
        }
    }
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

/// SPEC.md: "Stop, Restart, Replace, Clone, Delete and the tunnel pill remain fully visible and keyboard reachable from
/// a 650px main pane". The pill is the first thing in the top row's left region, so it is what can push the actions,
/// and with a long host name it is wide: it must give up its width first, cutting the host short and keeping both
/// ports. With the text size group at the start of the actions, the pill at its widest no longer fits beside them, so
/// it gives up width, down to no less than its floor for a mapping (154 pixels). Specified:
/// in a 650-pixel pane, with a session on a remote host whose terminal lists one tunnel on a host with a very long
/// name, each of the four actions is drawn whole inside the pane, all on one line, and the pill is drawn whole inside
/// the pane too, tracking a focus handle so Tab reaches it.
#[test]
fn the_tunnel_pill_and_every_action_fit_a_650_pixel_pane() {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        assets::UI_FONT,
    ));
    let mut cx =
        HeadlessAppContext::with_platform(text_system, Arc::new(assets::Assets), || Ok(None));
    cx.update(bootstrap::install).unwrap();
    let (mut fake, handle, events) = FakeHelm::new();
    let _connection = cx.update(|cx| crate::session::start(handle, events, None, cx));
    cx.run_until_parked();
    // The session on the pinned host list's SSH host 2, so its terminal reads its tunnels as it opens.
    let mut remote = entry(&idle("a"), 0);
    remote.value["host"] = json!(2);
    let hosts: Value = serde_json::from_str(include_str!(
        "../../../../farhelm-helm/http-contract/host-list.json"
    ))
    .unwrap();
    let host_entries: Vec<KeyedEntry> = hosts["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|host| KeyedEntry {
            key: farhelm_proto::ui_channel::HostView::key(host["id"].as_i64().unwrap()),
            value: host.clone(),
        })
        .collect();
    let keys: Vec<String> = host_entries.iter().map(|entry| entry.key.clone()).collect();
    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    let mut snapshot = snapshot_of(vec![remote], false);
    snapshot.push(CollectionState {
        collection: Collection::Hosts,
        meta: meta(&keys),
        entries: host_entries,
    });
    fake.connect(snapshot);
    cx.run_until_parked();
    let window = cx
        .open_window(size(px(650.), px(300.)), |window, cx| {
            let view = cx.new(|cx| {
                let pane = cx.new(|cx| SessionPane::new(window, cx));
                PaneOnly {
                    pane,
                    _list: cx.new(SessionList::new),
                    root: root::attach(window, cx),
                }
            });
            cx.new(|cx| Root::new(view, window, cx))
        })
        .unwrap();
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
    draw(&mut cx);
    // Answer what the app sent, in order: the terminal's tunnel read lists one tunnel on a very long host name.
    let long_host = format!("a-very-long-host-name{}.example.internal", "-x".repeat(40));
    let mut read = false;
    for command in fake.commands() {
        if command.method == Method::Get && command.path == "/api/sessions/a/tunnels" {
            read = true;
            fake.reply(
                command.id,
                200,
                Some(json!({
                    "available": true,
                    "host_name": long_host,
                    "consent": true,
                    "tunnels": [{ "id": "t1", "local_port": 54321, "remote_port": 5173 }],
                })),
            );
        } else {
            fake.reply(command.id, 200, None);
        }
    }
    assert!(
        read,
        "the premise: the remote session's terminal read its tunnels"
    );
    let nodes = draw(&mut cx);
    assert_whole_on_one_line(&nodes);
    let pill = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("session-tunnel-pill"))
        .expect("the pill is drawn");
    assert!(
        pill.bounds.size.width < px(280.) && pill.bounds.size.width >= px(154.),
        "the pill gave way to the actions, down to no less than its floor: {:?}",
        pill.bounds.size.width
    );
    assert!(
        pill.bounds.size.width > px(0.)
            && pill.visible_bounds == pill.bounds
            && pill.bounds.origin.x + pill.bounds.size.width <= px(650.),
        "the pill is drawn whole inside the pane: {:?}, visible {:?}",
        pill.bounds,
        pill.visible_bounds
    );
    assert!(
        pill.focus.is_some(),
        "and tracks a focus handle, so Tab reaches it"
    );
}

// ---- The terminal text size ----------------------------------------------------------------------------------------

/// A snapshot with session `a` open and the shared `terminal_text_size` preference at `points` (unset for `None`).
fn sized(points: Option<u32>) -> Vec<CollectionState> {
    let mut snapshot = snapshot_of(vec![entry(&session("a"), 0)], false);
    if let Some(points) = points {
        snapshot[1].entries[0].value["terminal_text_size"] = json!(points);
    }
    snapshot
}

/// Push the shared `terminal_text_size` preference at `points`, as the helm does once it stores a write, with session
/// `a` still the last visited.
fn push_size(fake: &mut FakeHelm, points: u32, cx: &mut VisualTestContext) {
    let mut preferences = preferences_entry(&["a"], false);
    preferences.value["terminal_text_size"] = json!(points);
    fake.push(CollectionChanges {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        upsert: vec![preferences],
        remove: Vec::new(),
    });
    test_support::settle(cx);
}

/// The size the group's readout paints.
fn readout(cx: &mut VisualTestContext) -> String {
    let nodes = frame(cx);
    let at = nodes
        .iter()
        .position(|node| node.element_id.as_deref() == Some(crate::terminal::text_size::READOUT_ID))
        .expect("the readout is drawn");
    nodes
        .iter()
        .enumerate()
        .filter(|(index, node)| *index == at || node.parent == Some(at))
        .flat_map(|(_, node)| node.texts.iter().map(|text| text.text.to_string()))
        .collect()
}

/// The text size writes the window sent since the last call, by their ids and sizes, left unanswered; anything else it
/// sent (a seen mark) is answered with success at once.
fn size_writes(fake: &mut FakeHelm) -> Vec<(u64, Value)> {
    let mut found = Vec::new();
    for command in fake.commands() {
        let body = command.body.clone().unwrap_or_default();
        if command.method == Method::Put
            && command.path == "/api/preferences"
            && body.get("terminal_text_size").is_some()
        {
            found.push((command.id, body["terminal_text_size"].clone()));
        } else {
            fake.reply(command.id, 200, None);
        }
    }
    found
}

/// SPEC.md, "Terminal experience": the magnifier − / + buttons "step every open terminal's text size together ...
/// within a fixed range", with the display-only readout between them following every change; the size is the helm's
/// shared preference, unset meaning 14 points. The size must show the step at once and survive the helm's answer, or a
/// press would flicker back until the push lands, and a held key would step from a stale size. Specified: with the
/// preference unset the readout shows 14; Larger sends `terminal_text_size: 15` and shows 15 before any answer; with
/// the helm's push of 15 and its answer it still shows 15; two quick Smaller presses send 14 then 13 and show 13; the
/// answer to the first alone leaves 13 shown; the push of 13 and the second answer leave 13.
#[gpui::test]
fn the_text_size_buttons_step_the_shared_size_and_the_readout_follows(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, sized(None));
    size_writes(&mut fake);
    assert_eq!(readout(cx), "14", "unset is the old default");

    test_support::click(cx, crate::terminal::text_size::LARGER_ID);
    let writes = size_writes(&mut fake);
    assert_eq!(
        writes
            .iter()
            .map(|(_, size)| size.clone())
            .collect::<Vec<_>>(),
        [json!(15)]
    );
    assert_eq!(readout(cx), "15", "the step shows before the helm answers");
    push_size(&mut fake, 15, cx);
    fake.reply(writes[0].0, 204, None);
    test_support::settle(cx);
    assert_eq!(readout(cx), "15");

    test_support::click(cx, crate::terminal::text_size::SMALLER_ID);
    test_support::click(cx, crate::terminal::text_size::SMALLER_ID);
    let writes = size_writes(&mut fake);
    assert_eq!(
        writes
            .iter()
            .map(|(_, size)| size.clone())
            .collect::<Vec<_>>(),
        [json!(14), json!(13)],
        "the second press steps from the first's size, not the stored one"
    );
    assert_eq!(readout(cx), "13");
    fake.reply(writes[0].0, 204, None);
    test_support::settle(cx);
    assert_eq!(
        readout(cx),
        "13",
        "an answer to an earlier step leaves the latest held"
    );
    push_size(&mut fake, 13, cx);
    fake.reply(writes[1].0, 204, None);
    test_support::settle(cx);
    assert_eq!(readout(cx), "13");
}

/// A refused or failed write must not leave the terminals at a size the helm never stored, which every other client
/// and the next start would disagree with: the hold ends with the helm's answer whatever it is, and the size falls back
/// to the stored one. Specified: at a stored 16, Larger shows 17; the helm's refusal brings back 16 and nothing more is
/// sent.
#[gpui::test]
fn a_refused_text_size_falls_back_to_the_stored_one(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, sized(Some(16)));
    size_writes(&mut fake);
    assert_eq!(readout(cx), "16");
    test_support::click(cx, crate::terminal::text_size::LARGER_ID);
    let writes = size_writes(&mut fake);
    assert_eq!(writes.len(), 1);
    assert_eq!(readout(cx), "17");
    fake.reply(
        writes[0].0,
        400,
        Some(json!({"error": "terminal_text_size out of range"})),
    );
    test_support::settle(cx);
    assert_eq!(readout(cx), "16", "back to what the helm stores");
    assert_eq!(size_writes(&mut fake), []);
}

/// The range is fixed (9 to 28 points, the glyph atlas's bound), and the helm refuses a size outside it, so a step past
/// an end must send nothing rather than a write certain to be refused. A stored size outside the range (an older or
/// foreign client) draws at the default rather than at a size the terminal was never meant to take. Specified: at 28,
/// Larger sends nothing and the readout stays 28; at 9, Smaller sends nothing and it stays 9; a stored 40 reads 14.
#[gpui::test]
fn the_text_size_stops_at_the_ends_of_its_range(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, sized(Some(28)));
    size_writes(&mut fake);
    test_support::click(cx, crate::terminal::text_size::LARGER_ID);
    assert_eq!(size_writes(&mut fake), [], "nothing past the top");
    assert_eq!(readout(cx), "28");
    push_size(&mut fake, 9, cx);
    assert_eq!(readout(cx), "9");
    test_support::click(cx, crate::terminal::text_size::SMALLER_ID);
    assert_eq!(size_writes(&mut fake), [], "nothing past the bottom");
    assert_eq!(readout(cx), "9");
    push_size(&mut fake, 40, cx);
    assert_eq!(
        readout(cx),
        "14",
        "a stored size out of range draws at the default"
    );
}

/// SPEC.md, "Terminal experience": "While a terminal is open the shortcut works wherever focus is"; with none open it
/// has nothing to size and must not write the shared preference behind the user's back. The chord's binding is the
/// physical-key table's (its own tests); this is what the action does once dispatched. Specified: with session `a`'s
/// terminal open, the Larger and Smaller actions dispatched to the window send 15 and then 14 (stepping from the held
/// size). The window without a terminal is the next test.
#[gpui::test]
fn the_text_size_shortcut_steps_only_while_a_terminal_is_open(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, sized(None));
    size_writes(&mut fake);
    assert!(
        cx.update(|_, cx| crate::terminal::text_size::any_terminal_open(cx)),
        "the premise: a terminal is open"
    );
    cx.dispatch_action(crate::actions::IncreaseTerminalTextSize);
    test_support::settle(cx);
    cx.dispatch_action(crate::actions::DecreaseTerminalTextSize);
    test_support::settle(cx);
    assert_eq!(
        size_writes(&mut fake)
            .into_iter()
            .map(|(_, size)| size)
            .collect::<Vec<_>>(),
        [json!(15), json!(14)]
    );
}

/// [`the_text_size_shortcut_steps_only_while_a_terminal_is_open`]'s other half, in a window with no session open, so
/// no terminal: the shortcut sends nothing.
#[gpui::test]
fn the_text_size_shortcut_does_nothing_without_a_terminal(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot_of(Vec::new(), false));
    size_writes(&mut fake);
    assert!(
        !cx.update(|_, cx| crate::terminal::text_size::any_terminal_open(cx)),
        "the premise: no terminal is open"
    );
    cx.dispatch_action(crate::actions::IncreaseTerminalTextSize);
    test_support::settle(cx);
    assert_eq!(size_writes(&mut fake), []);
}

/// A terminal opened while the size is not the default must start at that size: it takes a size change only when the
/// size moves, so one measured at the default would stay at 14 points while the readout said otherwise, until the next
/// step. Specified: with the shared preference stored at 20, the open session's terminal fits and draws its grid at the
/// font measured at 20 points.
#[gpui::test]
fn a_terminal_opens_at_the_current_text_size(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, sized(Some(20)));
    let (opened, wanted) = cx.update(|_, cx| {
        let opened: Vec<_> = crate::terminal::text_size::open_terminals(cx)
            .iter()
            .map(|terminal| terminal.read(cx).metrics())
            .collect();
        let wanted = crate::terminal::CellMetrics::measure(crate::terminal::TextSize::new(20), cx);
        (opened, wanted)
    });
    assert_eq!(opened.len(), 1, "the premise: the open session's terminal");
    assert!(Some(opened[0]) == wanted, "it opened at 20 points");
}

/// The old app's limit-button case: a press on a size button at the end of the range once dropped the terminal's focus,
/// so the next typing went nowhere. Here a click focuses a control only where the platform's rule says so (SPEC.md,
/// "Keyboard focus"); under the Mac's rule the terminal must keep focus through any press, at a limit or not, so typing
/// goes on reaching the program. Specified: with clicks not focusing controls and the terminal focused, Larger at 28
/// (sending nothing) and then Smaller (stepping to 27) each leave focus on the terminal.
#[gpui::test]
fn a_text_size_press_leaves_focus_on_the_terminal_under_the_macs_click_rule(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, sized(Some(28)));
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    size_writes(&mut fake);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("terminal-grid"),
        "the premise: the terminal is home"
    );
    test_support::click(cx, crate::terminal::text_size::LARGER_ID);
    assert_eq!(size_writes(&mut fake), [], "the premise: at the limit");
    assert_eq!(focused_id(cx).as_deref(), Some("terminal-grid"));
    test_support::click(cx, crate::terminal::text_size::SMALLER_ID);
    assert_eq!(size_writes(&mut fake).len(), 1);
    assert_eq!(focused_id(cx).as_deref(), Some("terminal-grid"));
}

/// SPEC_impl.md, "The store, and how views redraw": a step held ahead of the store belongs to the UI session, and a
/// reset forgets it, since the reset drops the reply that would end the hold. A hold that outlived the reset would keep
/// every terminal at a size the helm may never have stored, with nothing left to release it. Specified: Larger shows
/// 15 with its write unanswered; after the channel breaks and the reconnect's snapshot still has the size unset, the
/// readout shows 14 and the reopened terminal draws at the font measured at 14 points.
#[gpui::test]
fn a_reset_forgets_an_unanswered_text_size_step(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, sized(None));
    size_writes(&mut fake);
    test_support::click(cx, crate::terminal::text_size::LARGER_ID);
    assert_eq!(
        size_writes(&mut fake).len(),
        1,
        "the premise: one write, unanswered"
    );
    assert_eq!(readout(cx), "15");

    fake.break_channel();
    test_support::settle(cx);
    fake.connect(sized(None));
    test_support::settle(cx);
    size_writes(&mut fake);
    assert_eq!(readout(cx), "14", "the reset forgot the step");
    let (newest, wanted) = cx.update(|_, cx| {
        let newest = crate::terminal::text_size::open_terminals(cx)
            .last()
            .map(|terminal| terminal.read(cx).metrics());
        let wanted = crate::terminal::CellMetrics::measure(crate::terminal::TextSize::DEFAULT, cx);
        (newest, wanted)
    });
    assert!(
        newest.is_some() && newest == wanted,
        "the reopened terminal draws at 14 points"
    );
}

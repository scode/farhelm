//! Tests of the open session's top row: the actions' order, labels and hover help, Restart's offer and its request,
//! that Stop, Replace and Delete from the header ask the row's questions and send the row's preconditions, a pending
//! Delete and Stop, exclusion, where Escape and Cancel leave focus, the row's layout at the 650-pixel floor with the
//! real text system (its actions condensed beside a tunnel pill there, and not in a wide pane, and the tab strip's
//! floor beside the pill), and the terminal text size group at the start of the actions (its steps, readout, range,
//! shortcut, the size a terminal opens at, and its height beside the buttons in both looks). The header of an open
//! session the list does not show is
//! `carried_tests.rs`'s.
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
use super::header::{Action, Control, TABS_SLOT};
use crate::actions::{DELETE_SESSION_CHORD, DELETE_SESSION_SHORTCUT_NAME};
use crate::focus::Mark;
use crate::launcher::launch_tests::{HostChange, start_host_change};
use crate::main_view::MainView;
use crate::session_list::SessionList;
use crate::session_list::lifecycle_tests::{
    consented, drawn_inside, node_bounds, refuse_for_yolo, replace,
};
use crate::session_list::menu_tests::{any_layer, drawn, ended, entry, label_of};
use crate::session_list::outcome::{self, PROGRESS_SHOWS_AFTER};
use crate::session_list::tests::{
    Fixture, change, launcher_drawn, listing_meta, open_main, painted, selected, session,
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

/// SPEC.md, "Terminal experience": the top row holds, on the right, Stop, Restart, Replace, Clone and Delete, with the
/// terminal area's tabs on the left. Restart is named for its offer, as the old button's `aria-label` was, because
/// "Restart" alone does not say whether the conversation survives. Specified: the five buttons are drawn left to right
/// in that order, after the empty tabs region; they paint "Stop", "Restart", "Replace", "Clone" and "Delete" and are
/// named "Stop", "Resume conversation", "Replace", "Clone" and "Delete". The carets between them are
/// `clone_and_the_split_carets_sit_in_the_old_order_with_the_old_words`'s.
#[gpui::test]
fn the_row_draws_stop_restart_replace_clone_and_delete_in_the_old_order(cx: &mut TestAppContext) {
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
        ["Stop", "Resume conversation", "Replace", "Clone", "Delete"]
            .map(|name| Some(name.to_owned()))
    );
    let painted = painted(&nodes);
    for label in ["Stop", "Restart", "Replace", "Clone", "Delete"] {
        assert!(
            painted.iter().any(|text| text == label),
            "{label} is painted"
        );
    }
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

/// Restart and Replace start sessions, so they are launches, and SPEC.md, "Creation", says "Launches and changes to the
/// host registry still exclude each other": a launch that picked its host while the registry was changing could aim at
/// an entry the change is about to rewrite. The migration's orchestrator decided on 2026-11-15 that they wait for a
/// host change as the launcher's Launch does, giving its reason; this pins both surfaces that offer them, which a later
/// change could otherwise leave out of the exclusion one at a time.
/// Specified: with Add host's probe in flight, the header's Restart and Replace and the row menu's Replace each have
/// "Wait for the host change to finish" as their hover help, a click on each opens nothing, and nothing is sent; once
/// the probe is answered, the header's Replace has its own help again, its Restart sends the idle session's Restart,
/// and the menu's Replace asks and sends its Replace.
#[gpui::test]
fn restart_and_replace_wait_for_a_host_change(cx: &mut TestAppContext) {
    const WAIT: &str = "Wait for the host change to finish";
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&idle("a"), 0)], false));
    pending(&mut fake);
    let probe = probe_alone(&mut fake, cx);

    for button in ["session-header-restart", "session-header-replace"] {
        assert_eq!(hover(cx, button).as_deref(), Some(WAIT), "{button}");
        test_support::click(cx, button);
        assert!(!any_layer(cx), "{button} asks nothing");
    }
    test_support::click(cx, "session-menu-a");
    assert_eq!(hover(cx, "session-menu-replace").as_deref(), Some(WAIT));
    test_support::click(cx, "session-menu-replace");
    // What tells an unavailable item from an enabled one that asked nothing: the enabled one closes the menu.
    assert!(
        drawn(cx, "session-menu"),
        "the unavailable Replace ignored the click; the menu is still open"
    );
    assert!(!drawn(cx, "prompt"), "the menu's Replace asks nothing");
    test_support::keys(cx, "escape");
    let meanwhile = fake.commands();
    assert!(
        meanwhile.iter().all(|command| !is_lifecycle(command)),
        "nothing was sent during the probe"
    );

    // The fake answers in order: the probe first, then whatever else went out behind it.
    fake.reply(probe, 200, Some(json!({})));
    for command in meanwhile {
        fake.reply(command.id, 200, None);
    }
    test_support::settle(cx);
    assert!(
        !cx.update(|_, cx| crate::hosts::host_change_in_flight(cx)),
        "the premise: the host change is over"
    );
    // Off the buttons and back, so the tooltip is shown afresh with the help drawn now.
    let away = test_support::center(cx, TABS_SLOT);
    cx.simulate_mouse_move(away, None, Modifiers::none());
    test_support::settle(cx);
    assert_eq!(
        hover(cx, "session-header-replace").as_deref(),
        Some("Replace: swap this session for a fresh conversation with the same settings")
    );
    test_support::click(cx, "session-header-restart");
    assert!(!any_layer(cx), "the idle agent restarts unasked");
    assert_eq!(answered(&mut fake), [restart("a", false)]);
    test_support::click(cx, "session-menu-a");
    test_support::click(cx, "session-menu-replace");
    assert!(drawn(cx, "prompt"), "the menu's Replace asks");
    test_support::click(cx, "prompt-confirm");
    assert_eq!(answered(&mut fake), [replace("a", false, false)]);
}

/// Start Add host's probe with nothing else outstanding, and return its envelope id, to answer it by: the fake answers
/// in order, so the test must answer the probe before anything sent after it.
fn probe_alone(fake: &mut FakeHelm, cx: &mut VisualTestContext) -> u64 {
    let path = start_host_change(cx, HostChange::Probe);
    let sent = fake.commands();
    let [probe] = &sent[..] else {
        panic!(
            "the premise: only the probe went out, not {} commands",
            sent.len()
        );
    };
    assert_eq!(probe.path, path, "the premise: the probe is in flight");
    probe.id
}

/// SPEC.md, "Errors and diagnostics": "Every failed operation surfaces a concrete, actionable error", and no dialog
/// may close as though it succeeded. A question that opened before a host change and is confirmed during one cannot
/// send its launch (Restart and Replace wait for host changes), so the refusal must show where the session's refusals
/// do; before this, the question closed and nothing said why. The inline YOLO question makes this easy to reach, since
/// it stays open while the user goes on to Add host. Specified: with a working session's Restart question open, a
/// probe started and the question confirmed, nothing is sent and the header and the row say "Restart: Wait for the
/// host change to finish"; once the probe is answered, the same with Replace's question and a second probe says
/// "Replace: Wait for the host change to finish".
#[gpui::test]
fn a_question_confirmed_during_a_host_change_says_why(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&session("a"), 0)], false));
    pending(&mut fake);
    for (button, said) in [
        (
            "session-header-restart",
            "Restart: Wait for the host change to finish",
        ),
        (
            "session-header-replace",
            "Replace: Wait for the host change to finish",
        ),
    ] {
        test_support::click(cx, button);
        assert!(drawn(cx, "prompt"), "the premise: {button} asks");
        let probe = probe_alone(&mut fake, cx);
        assert!(drawn(cx, "prompt"), "the premise: the question stays open");
        test_support::click(cx, "prompt-confirm");
        let meanwhile = fake.commands();
        assert!(
            meanwhile.iter().all(|command| !is_lifecycle(command)),
            "{button}: nothing was sent"
        );
        for line in ["session-header-outcome-a", "session-outcome-a"] {
            assert_eq!(label_of(cx, line).as_deref(), Some(said), "{line}");
        }
        fake.reply(probe, 200, Some(json!({})));
        for command in meanwhile {
            fake.reply(command.id, 200, None);
        }
        test_support::settle(cx);
    }
}

/// While a host change is in flight only the launches wait: Stop and Delete start nothing, and a Restart that cannot
/// resume would stay unavailable after the change, so its own reason is the one worth reading. A wider wait would block
/// ending a runaway agent for the length of a probe, or hide why Restart never becomes available. Specified: with a
/// probe in flight on a running session with one tab whose conversation was not captured, Stop's and Delete's help are
/// their own, Restart's is its "Restart unavailable" reason, and Stop and Delete still ask their questions (the tab is
/// what makes Stop ask, so that asking is what shows it was not refused).
#[gpui::test]
fn only_the_launches_wait_for_a_host_change(cx: &mut TestAppContext) {
    let row = offering(&session("a"), 1, "not_captured", "codex");
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![row], false));
    pending(&mut fake);
    let _probe = probe_alone(&mut fake, cx);
    assert_eq!(
        hover(cx, "session-header-stop").as_deref(),
        Some("Stop the agent and close its terminal tabs")
    );
    assert_eq!(
        hover(cx, "session-header-delete"),
        Some(format!(
            "Delete: remove this session and its state, after you confirm ({DELETE_SESSION_SHORTCUT_NAME})"
        ))
    );
    let restart = hover(cx, "session-header-restart").unwrap_or_default();
    assert!(
        restart.starts_with("Restart unavailable — "),
        "Restart keeps its own reason: {restart}"
    );
    for button in ["session-header-stop", "session-header-delete"] {
        test_support::click(cx, button);
        assert!(drawn(cx, "prompt"), "{button} asks during the probe");
        test_support::keys(cx, "escape");
    }
}

/// The one-second clock keeps the app waking every second, so the waiting lines must let go of it once nothing counts;
/// a clock kept after its request settled would wake the app every second for the rest of the session. Specified: an
/// idle session's Restart sent and unanswered keeps one clock and no one-second clock before two seconds, takes it at
/// two seconds, and a second after the reply keeps neither.
#[gpui::test]
fn the_one_second_clock_goes_with_the_last_line(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&idle("a"), 0)], false));
    pending(&mut fake);
    test_support::click(cx, "session-header-restart");
    let [(id, _, _)] = pending(&mut fake)[..] else {
        panic!("one Restart was sent");
    };
    let kept = |cx: &mut VisualTestContext| cx.update(|_, cx| outcome::kept(cx));
    assert_eq!(kept(cx), Some((false, 1)), "counting nothing yet");
    test_support::advance(cx, PROGRESS_SHOWS_AFTER);
    assert_eq!(kept(cx), Some((true, 1)), "counting from two seconds");
    fake.reply(id, 200, Some(json!({})));
    test_support::settle(cx);
    test_support::advance(cx, Duration::from_secs(1));
    assert_eq!(kept(cx), Some((false, 0)), "nothing kept once it settled");
}

/// The old `session-view-claims.spec.ts`: an agent or the command line deleting the open session while its header
/// Restart question is open, or while its Restart runs, must leave navigation working, and must not leave the next
/// session's header unavailable because of the gone session's question or request. Specified: with a's Restart question
/// open, a complete listing pushed without a closes the question and opens b, whose header's Replace then asks; and
/// with b's Restart sent and unanswered, a complete listing without b opens c, whose header's Replace asks too, while
/// b's Restart is still in flight.
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
        Some(Control::Action(Action::Replace)),
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

// ---- Clone, and the split buttons' carets ---------------------------------------------------------------------------

/// Restart's caret's id.
const RESTART_CARET: &str = "session-header-restart-options";
/// Replace's caret's id.
const REPLACE_CARET: &str = "session-header-replace-options";
/// Both carets, in the row's order.
const CARETS: [&str; 2] = [RESTART_CARET, REPLACE_CARET];

/// The accessible name of the launcher's main action, which says what a press does (Launch, Replace, Restart or "Stop
/// and restart", then the target): how these tests tell Clone, Replace with and Restart with apart.
fn launch_action(cx: &mut VisualTestContext) -> String {
    label_of(cx, "launcher-launch").expect("the launcher is drawn")
}

/// SPEC.md, "Terminal experience": the right of the top row is "Stop, Restart ▾, Replace ▾, Clone and Delete", the two
/// split buttons each a main part and a caret. The caret is a control of its own, so it needs a name that says whose
/// options it opens, and every control has hover help (the old caret's and the old header Clone's tooltips, pinned).
/// Specified: Stop, Restart, Restart's caret, Replace, Replace's caret, Clone and Delete are drawn left to right; each
/// caret starts where its main part ends and has the same height; the carets are named "Restart options" and "Replace
/// options" and Clone "Clone", which is painted; the carets' and Clone's hover help.
#[gpui::test]
fn clone_and_the_split_carets_sit_in_the_old_order_with_the_old_words(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&session("a"), 0)], false));
    pending(&mut fake);
    let nodes = frame(cx);
    let at = |id: &str| {
        nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("{id} was drawn"))
            .clone()
    };
    let order = [
        Action::Stop.id(),
        Action::Restart.id(),
        RESTART_CARET,
        Action::Replace.id(),
        REPLACE_CARET,
        Action::Clone.id(),
        Action::Delete.id(),
    ];
    let lefts: Vec<f32> = order
        .iter()
        .map(|id| f32::from(at(id).bounds.origin.x))
        .collect();
    assert!(
        lefts.windows(2).all(|pair| pair[0] < pair[1]),
        "left to right: {lefts:?}"
    );
    for (main, caret) in [
        (Action::Restart.id(), RESTART_CARET),
        (Action::Replace.id(), REPLACE_CARET),
    ] {
        let (main, caret) = (at(main).bounds, at(caret).bounds);
        assert_eq!(caret.left(), main.right(), "{caret:?} joins {main:?}");
        assert_eq!(caret.size.height, main.size.height, "one height");
    }
    assert_eq!(at(RESTART_CARET).label.as_deref(), Some("Restart options"));
    assert_eq!(at(REPLACE_CARET).label.as_deref(), Some("Replace options"));
    assert_eq!(at(Action::Clone.id()).label.as_deref(), Some("Clone"));
    assert!(painted(&nodes).iter().any(|text| text == "Clone"));
    assert_eq!(
        hover(cx, RESTART_CARET).as_deref(),
        Some("Restart options: use the current settings or edit them first")
    );
    assert_eq!(
        hover(cx, REPLACE_CARET).as_deref(),
        Some("Replace options: use the current settings or edit them first")
    );
    assert_eq!(
        hover(cx, "session-header-clone").as_deref(),
        Some("Clone: open the new-session form with these settings; this session keeps running")
    );
}

/// SPEC.md: "Split menus follow the app's shared keyboard rules: ... arrows and Home/End move between choices ...
/// Escape or Tab closes the menu and returns focus to its caret", and the old caret opened its menu with Down and Up.
/// Tab returning to the caret is these menus' own rule (every other menu keeps Tab inside), so the next Tab goes on to
/// the row's next control as if the menu had never opened. Specified, under the Linux click rule: a click on Restart's
/// caret opens the menu below it with focus on Restart, its header saying "Resume a conversation in this session", its
/// choices named "Restart" and "Restart with…"; Down, Down, Up, Home and End move focus to the variant, the plain
/// choice, the variant, the plain choice and the variant; Escape closes it with focus on the caret; Up on the closed
/// caret opens it on the variant; Tab closes it with focus on the caret, and a second Tab moves to Replace; Shift+Tab
/// comes back to the caret, Enter opens the menu on Restart, and Shift+Tab closes it with focus on the caret again.
/// Nothing is sent throughout.
#[gpui::test]
fn a_caret_menu_walks_its_choices_and_escape_or_tab_returns_to_the_caret(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&session("a"), 0)], false));
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    pending(&mut fake);
    let plain = "session-header-restart-options-plain";
    let with = "session-header-restart-options-with";

    test_support::click(cx, RESTART_CARET);
    let caret = test_support::bounds(cx, RESTART_CARET);
    let menu = test_support::bounds(cx, "session-header-restart-options-menu");
    assert!(
        menu.top() >= caret.bottom(),
        "{menu:?} hangs under {caret:?}"
    );
    assert_eq!(focused_id(cx).as_deref(), Some(plain));
    assert_eq!(label_of(cx, plain).as_deref(), Some("Restart"));
    assert_eq!(label_of(cx, with).as_deref(), Some("Restart with…"));
    assert!(
        texts(cx)
            .iter()
            .any(|text| text == "Resume a conversation in this session")
    );
    for (key, target) in [
        ("down", with),
        ("down", plain),
        ("up", with),
        ("home", plain),
        ("end", with),
    ] {
        test_support::keys(cx, key);
        assert_eq!(focused_id(cx).as_deref(), Some(target), "after {key}");
    }
    test_support::keys(cx, "escape");
    assert!(!any_layer(cx));
    assert_eq!(focused_id(cx).as_deref(), Some(RESTART_CARET), "Escape");

    test_support::keys(cx, "up");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(with),
        "Up opens on the variant"
    );
    test_support::keys(cx, "tab");
    assert!(!any_layer(cx), "Tab closed the menu");
    assert_eq!(focused_id(cx).as_deref(), Some(RESTART_CARET), "Tab");
    test_support::keys(cx, "tab");
    assert_eq!(focused_id(cx).as_deref(), Some("session-header-replace"));

    test_support::keys(cx, "shift-tab");
    assert_eq!(focused_id(cx).as_deref(), Some(RESTART_CARET));
    test_support::keys(cx, "enter");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(plain),
        "Enter opens on Restart"
    );
    test_support::keys(cx, "shift-tab");
    assert!(!any_layer(cx));
    assert_eq!(focused_id(cx).as_deref(), Some(RESTART_CARET), "Shift+Tab");
    assert_eq!(pending(&mut fake), []);
}

/// The menu's plain choice is the main part's action, not a copy of it: the same ask-or-send rule, the same question
/// hung under the main part (the old menu called the same handler), so the two cannot warn about different things.
/// Specified: on a running session, Restart's caret menu's Restart closes the menu and asks "Still running —
/// restarting stops the agent and its whole process tree first:" below the Restart button, nothing sent; Cancel
/// returns focus to the caret (where it was when the question opened); Replace's caret menu's Replace asks below the
/// Replace button, and Confirm replace sends the Replace.
#[gpui::test]
fn a_menus_plain_choice_asks_the_main_parts_question(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&session("a"), 0)], false));
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    pending(&mut fake);
    test_support::click(cx, RESTART_CARET);
    test_support::click(cx, "session-header-restart-options-plain");
    assert!(
        !drawn(cx, "session-header-restart-options-menu"),
        "the menu closed"
    );
    assert!(drawn(cx, "prompt"), "Restart asked");
    let button = test_support::bounds(cx, "session-header-restart");
    assert!(test_support::bounds(cx, "prompt").top() >= button.bottom());
    assert!(texts(cx).iter().any(|text| text
        == "Still running — restarting stops the agent and its whole process tree first:"));
    assert_eq!(pending(&mut fake), []);
    test_support::click(cx, "prompt-cancel");
    assert_eq!(focused_id(cx).as_deref(), Some(RESTART_CARET));

    test_support::click(cx, REPLACE_CARET);
    test_support::click(cx, "session-header-replace-options-plain");
    assert!(drawn(cx, "prompt"), "Replace asked");
    let button = test_support::bounds(cx, "session-header-replace");
    assert!(test_support::bounds(cx, "prompt").top() >= button.bottom());
    test_support::click(cx, "prompt-confirm");
    let [(_, path, _)] = &pending(&mut fake)[..] else {
        panic!("one Replace was sent");
    };
    assert_eq!(path, "/api/sessions/a/replace");
}

/// The "with…" choices and Clone are the header's ways into the launcher (its Clone, Replace with and Restart with),
/// and SPEC.md has "Cancelling Restart with returns to that persistent caret"; Clone's launcher returns to Clone.
/// Specified, under the Linux click rule on a running session: Restart with… closes the menu and opens the launcher in
/// restart mode (its action "Stop and restart …", the agent being at work); Escape closes it with focus on Restart's
/// caret; Replace with… opens it in Replace with mode (its action "Replace …"), Escape returning to Replace's caret;
/// Clone opens it with the session's title in its Name field and the action "Launch …", Escape returning to Clone.
/// No lifecycle request is sent.
#[gpui::test]
fn the_with_choices_and_clone_open_the_launcher_and_return_focus_on_cancel(
    cx: &mut TestAppContext,
) {
    // An ordinary folder session: Clone of one in a managed checkout names its new checkout `<title>-clone` instead
    // (`launcher::checkout_tests` covers that), and this test is about the header's way in, not the name.
    let a = session("a").unmanaged().not_launched_as_checkout();
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&a, 0)], false));
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    pending(&mut fake);
    for (caret, with, verb) in [
        (
            RESTART_CARET,
            "session-header-restart-options-with",
            "Stop and restart",
        ),
        (
            REPLACE_CARET,
            "session-header-replace-options-with",
            "Replace",
        ),
    ] {
        test_support::click(cx, caret);
        test_support::click(cx, with);
        assert!(launcher_drawn(cx), "{with} opened the launcher");
        assert!(!drawn(cx, "session-header-restart-options-menu"));
        let action = launch_action(cx);
        assert!(action.starts_with(verb), "{with}: {action}");
        test_support::keys(cx, "escape");
        assert!(!any_layer(cx));
        assert_eq!(focused_id(cx).as_deref(), Some(caret), "{with}");
    }

    test_support::click(cx, "session-header-clone");
    assert!(launcher_drawn(cx), "Clone opened the launcher");
    let action = launch_action(cx);
    assert!(action.starts_with("Launch"), "{action}");
    let name: Vec<String> =
        crate::session_list::host_rail::tests::texts_within(&frame(cx), "launcher-name")
            .into_iter()
            .map(|text| text.text.to_string())
            .collect();
    assert_eq!(name, ["a"], "the source's title");
    test_support::keys(cx, "escape");
    assert_eq!(focused_id(cx).as_deref(), Some("session-header-clone"));
    assert!(
        fake.commands().iter().all(|command| !is_lifecycle(command)),
        "nothing was sent"
    );
}

/// SPEC.md: "An unavailable plain Restart does not hide an independently available Restart with; the menu remains
/// available to inspect reasons", and Restart with stays "visible but greyed out ... with a hover tooltip and
/// accessible description explaining why". So the caret opens even when nothing in it can run, and each choice keeps
/// its reason as its description and help. Specified: for a command launch without a resume command, Restart's caret
/// says "Neither option is available. Open the menu for the reasons."; its menu says "Unavailable for this session",
/// the plain choice's help is "This command launch did not opt into Resume, so it has no resume command and Replace
/// starts it over" and the variant's "This command launch has no resume command, so it cannot be restarted; use Replace
/// with…" (both also painted as descriptions); a click on either leaves the menu open and opens and sends nothing.
#[gpui::test]
fn an_unavailable_choice_stays_in_its_menu_with_its_reason(cx: &mut TestAppContext) {
    let mut row = offering(&session("a"), 0, "no_resume_command", "claude");
    row.value["launch"] = json!({"kind": "command", "command": "make watch", "yolo": false, "agent": null,
        "resume": null});
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![row], false));
    pending(&mut fake);
    assert_eq!(
        hover(cx, RESTART_CARET).as_deref(),
        Some("Neither option is available. Open the menu for the reasons.")
    );
    test_support::click(cx, RESTART_CARET);
    assert!(
        texts(cx)
            .iter()
            .any(|text| text == "Unavailable for this session")
    );
    let plain_reason = "This command launch did not opt into Resume, so it has no resume command and Replace starts \
                        it over";
    let with_reason =
        "This command launch has no resume command, so it cannot be restarted; use Replace with…";
    let painted = texts(cx);
    for reason in [plain_reason, with_reason] {
        assert!(
            painted.iter().any(|text| text == reason),
            "{reason} is painted"
        );
    }
    assert_eq!(
        hover(cx, "session-header-restart-options-plain").as_deref(),
        Some(plain_reason)
    );
    assert_eq!(
        hover(cx, "session-header-restart-options-with").as_deref(),
        Some(with_reason)
    );
    for choice in [
        "session-header-restart-options-plain",
        "session-header-restart-options-with",
    ] {
        test_support::click(cx, choice);
        assert!(
            drawn(cx, "session-header-restart-options-menu"),
            "{choice} ignored the click; the menu is still open"
        );
        assert!(!launcher_drawn(cx));
    }
    assert_eq!(pending(&mut fake), []);
}

/// Clone and the "with…" choices open the launcher, which SPEC.md keeps from opening while a launch is pending or a
/// host change runs ("Launches and changes to the host registry still exclude each other"), so they are drawn
/// unavailable then, as New's button is, and say why: a greyed entry's "tooltips and accessible descriptions explain
/// the specific reason". Clone keeps its place in Tab's order, as Restart and Replace do while they wait. A host probe
/// stands in for both causes. Specified: with Add host's probe in flight, the header's Clone is still focusable, its
/// help is "Wait for the host change to finish", and a click on it opens nothing; Replace's caret still opens its menu,
/// whose two choices' help is "Wait for the host change to finish" and each of which ignores a click (the menu stays
/// open, the launcher closed, nothing sent); Restart's menu says the same for Restart with…; once the probe is
/// answered, Clone's help is its own again and it takes clicks.
#[gpui::test]
fn clone_and_the_with_choices_wait_while_a_host_change_runs(cx: &mut TestAppContext) {
    use crate::launcher::launch_tests::{HostChange, start_host_change};
    const WAIT: &str = "Wait for the host change to finish";
    let (mut fake, cx) = open_main(cx, snapshot_of(vec![entry(&session("a"), 0)], false));
    pending(&mut fake);
    let path = start_host_change(cx, HostChange::Probe);
    let probe = fake
        .commands()
        .into_iter()
        .find(|command| command.path == path)
        .expect("the premise: the probe is in flight")
        .id;
    let clone = |cx: &mut VisualTestContext| {
        frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some("session-header-clone"))
            .expect("Clone is drawn")
    };
    assert!(clone(cx).flags.focusable, "Clone stays keyboard-reachable");
    assert_eq!(hover(cx, "session-header-clone").as_deref(), Some(WAIT));
    test_support::click(cx, "session-header-clone");
    assert!(!launcher_drawn(cx), "a click on Clone opens nothing");

    test_support::click(cx, REPLACE_CARET);
    assert!(
        drawn(cx, "session-header-replace-options-menu"),
        "the caret opens"
    );
    for choice in [
        "session-header-replace-options-plain",
        "session-header-replace-options-with",
    ] {
        assert_eq!(hover(cx, choice).as_deref(), Some(WAIT), "{choice}");
        test_support::click(cx, choice);
        assert!(
            drawn(cx, "session-header-replace-options-menu") && !launcher_drawn(cx),
            "{choice} ignored the click"
        );
    }
    test_support::keys(cx, "escape");
    test_support::click(cx, RESTART_CARET);
    assert_eq!(
        hover(cx, "session-header-restart-options-with").as_deref(),
        Some(WAIT)
    );
    test_support::keys(cx, "escape");
    assert!(
        fake.commands().iter().all(|command| !is_lifecycle(command)),
        "nothing was sent"
    );

    fake.reply(probe, 200, Some(json!({})));
    test_support::settle(cx);
    let away = test_support::center(cx, TABS_SLOT);
    cx.simulate_mouse_move(away, None, Modifiers::none());
    test_support::settle(cx);
    assert_eq!(
        hover(cx, "session-header-clone").as_deref(),
        Some("Clone: open the new-session form with these settings; this session keeps running")
    );
    assert!(clone(cx).flags.click, "Clone takes clicks again");
}

/// The helm's pinned host list as a hosts collection, so a launcher opened from the header has a host to launch on.
fn hosts_state() -> CollectionState {
    let hosts: Value = serde_json::from_str(include_str!(
        "../../../../farhelm-helm/http-contract/host-list.json"
    ))
    .unwrap();
    let entries: Vec<KeyedEntry> = hosts["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|host| KeyedEntry {
            key: farhelm_proto::ui_channel::HostView::key(host["id"].as_i64().unwrap()),
            value: host.clone(),
        })
        .collect();
    let keys: Vec<String> = entries.iter().map(|entry| entry.key.clone()).collect();
    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    CollectionState {
        collection: Collection::Hosts,
        meta: meta(&keys),
        entries,
    }
}

/// The creates (`POST /api/sessions`) the window sent since the last call, round after round until a round sends
/// nothing else. Every other command is answered at once, the launcher's catalog read as unavailable, so that no
/// stored choice is refused for missing from a catalog.
fn creates(fake: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<FakeCommand> {
    let mut creates = Vec::new();
    loop {
        cx.run_until_parked();
        let mut answered = false;
        for command in fake.commands() {
            if command.path == "/api/sessions" {
                creates.push(command);
            } else if command.path.ends_with("launch-catalog") {
                fake.reply(
                    command.id,
                    503,
                    Some(json!("catalog unavailable in this test")),
                );
                answered = true;
            } else {
                fake.reply(command.id, 200, None);
                answered = true;
            }
        }
        test_support::settle(cx);
        if !answered {
            return creates;
        }
    }
}

/// The other cause of the launcher's wait: a launch the user has asked for and the helm has not answered. The launcher
/// holds one launch at a time, so Clone waits for it, and must say so rather than read as ready. Specified: on a
/// command launch's session with the helm's hosts listed, Clone from the header opens the launcher, and its Launch
/// sends one create, which stays unanswered; the header's Clone, still focusable, then says "Wait for the pending
/// launch to finish", and a click on it opens nothing.
#[gpui::test]
fn clone_says_why_it_waits_while_a_launch_is_pending(cx: &mut TestAppContext) {
    // An ordinary folder session, so that Launch sends its create at once: a Clone of one in a managed checkout opens on
    // a new checkout, whose Launch waits for a checkout preview this test does not answer.
    let mut row = entry(&session("a").unmanaged().not_launched_as_checkout(), 0);
    row.value["launch"] = json!({"kind": "command", "command": "make watch", "yolo": false, "agent": null,
        "resume": null});
    let mut snapshot = snapshot_of(vec![row], false);
    snapshot.push(hosts_state());
    let (mut fake, cx) = open_main(cx, snapshot);
    creates(&mut fake, cx);
    test_support::click(cx, "session-header-clone");
    assert!(launcher_drawn(cx), "the premise: Clone opened the launcher");
    assert!(
        creates(&mut fake, cx).is_empty(),
        "the premise: opening sends no create"
    );
    test_support::click(cx, "launcher-launch");
    let sent = creates(&mut fake, cx);
    assert_eq!(
        sent.len(),
        1,
        "the premise: one create is pending (the launcher says {:?})",
        label_of(cx, "launcher-refusal")
    );
    let clone = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("session-header-clone"))
        .expect("Clone is drawn");
    assert!(clone.flags.focusable, "Clone stays keyboard-reachable");
    assert_eq!(
        hover(cx, "session-header-clone").as_deref(),
        Some("Wait for the pending launch to finish")
    );
    test_support::click(cx, "session-header-clone");
    assert!(!launcher_drawn(cx), "a click on Clone opens nothing");
}

/// SPEC.md, "Keyboard focus": a menu goes away "when the thing it is about is gone (a deleted session's open menu,
/// say)". A caret menu is about the open session, and the command line or an agent can delete that session while the
/// menu is open; the next session then opens under the same carets, and a menu left open would act on the wrong
/// session or show nothing. Specified: with a's Restart caret menu open, a complete listing pushed without a opens b,
/// and the menu is no longer drawn nor any layer open.
#[gpui::test]
fn a_caret_menu_closes_when_its_session_is_deleted_elsewhere(cx: &mut TestAppContext) {
    let rows = vec![entry(&session("a"), 0), entry(&session("b"), 0)];
    let (mut fake, cx) = open_main(cx, snapshot_of(rows, false));
    pending(&mut fake);
    test_support::click(cx, RESTART_CARET);
    assert!(
        drawn(cx, "session-header-restart-options-menu"),
        "the premise: a's Restart menu is open"
    );
    fake.push(change(listing_meta(&["b"], 1, false), &[], &["a"]));
    test_support::settle(cx);
    test_support::redraw(cx);
    assert_eq!(selected(cx).as_deref(), Some("b"), "b opened");
    assert!(!drawn(cx, "session-header-restart-options-menu"));
    assert!(!any_layer(cx), "the menu went with its session");
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

/// Each of the five actions and both split carets in `nodes` is drawn whole (nothing of it clipped) inside a 650-pixel
/// pane, and all of them sit on one line; so are the text size group's two buttons and readout, which share that line
/// and whose buttons track a focus handle, so Tab reaches them (SPEC.md names text size first among what must stay
/// whole and reachable at 650 pixels, and "both parts of each split button" among the rest).
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
    let carets = CARETS.map(|id| {
        nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("{id} was drawn"))
    });
    let actions = Action::ALL.map(|action| drawn_node(nodes, action));
    for node in actions.iter().chain(carets.iter()) {
        let name = node.element_id.as_deref().unwrap_or_default();
        let bounds = node.bounds;
        assert!(
            bounds.size.width > px(0.) && node.visible_bounds == bounds,
            "{name} is drawn whole: {bounds:?}, visible {:?}",
            node.visible_bounds
        );
        assert!(
            bounds.origin.x >= px(0.) && bounds.origin.x + bounds.size.width <= px(650.),
            "{name} is inside the pane: {bounds:?}"
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
/// to draw. The pane gets that list's New handle, as the main view passes it. The list is not drawn, so neither is New,
/// and the focus coordinator counts an undrawn home as none; these layout tests' fleets are not empty, so New is never
/// home in them anyway.
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
/// (the tab strip that will is the terminal area's), so what this pins today is that the five actions and the two
/// carets fit whole on one line at 650 pixels; it becomes a real guard once the left region fills. The one thing that
/// could already push them is a long host name, which a Restart's wait line names: the line sits below the row, so it
/// must not. Specified: in a 650-pixel pane, with an idle session on a host with a long name, each of the five actions
/// and both carets is drawn whole inside the pane, all on one line, and each tracks a focus handle, so Tab reaches it;
/// once its Restart has waited three seconds (the wait line drawn), they are still drawn whole on one line.
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
                let list = cx.new(SessionList::new);
                let new = list.read(cx).new_focus().clone();
                let pane = cx.new(|cx| SessionPane::new(new, window, cx));
                kept.replace(Some(pane.clone()));
                PaneOnly {
                    pane,
                    _list: list,
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
    for caret in CARETS {
        let node = idle_nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(caret))
            .expect("the caret is drawn");
        assert!(node.focus.is_some(), "{caret} tracks a focus handle");
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

/// A session pane alone in a window of a given width, laid out with the real text system and the bundled fonts (text
/// width decides every fit here; the test context's fake text system gives every character the same width), over a
/// fake helm, with its first session open: the setup of the tests of the row under a tunnel pill.
struct RealPane {
    /// Declared first, so it drops before `cx`: GPUI's leak check, run as the app context drops, fails on a handle
    /// still held then.
    _connection: Entity<crate::session::Connection>,
    cx: HeadlessAppContext,
    fake: FakeHelm,
    window: gpui::AnyWindowHandle,
    /// The pane, weakly: a strong handle held as the app context drops would fail GPUI's leak check.
    pane: gpui::WeakEntity<SessionPane>,
}

impl RealPane {
    /// The pane `width` pixels wide over `snapshot`, not yet drawn.
    fn open(width: f32, snapshot: Vec<CollectionState>) -> RealPane {
        let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
            assets::UI_FONT,
        ));
        let mut cx =
            HeadlessAppContext::with_platform(text_system, Arc::new(assets::Assets), || Ok(None));
        cx.update(bootstrap::install).unwrap();
        let (mut fake, handle, events) = FakeHelm::new();
        let connection = cx.update(|cx| crate::session::start(handle, events, None, cx));
        cx.run_until_parked();
        fake.connect(snapshot);
        cx.run_until_parked();
        let built = Rc::new(RefCell::new(None));
        let kept = built.clone();
        let window = cx
            .open_window(size(px(width), px(300.)), |window, cx| {
                let view = cx.new(|cx| {
                    let list = cx.new(SessionList::new);
                    let new = list.read(cx).new_focus().clone();
                    let pane = cx.new(|cx| SessionPane::new(new, window, cx));
                    kept.replace(Some(pane.downgrade()));
                    PaneOnly {
                        pane,
                        _list: list,
                        root: root::attach(window, cx),
                    }
                });
                cx.new(|cx| Root::new(view, window, cx))
            })
            .unwrap();
        let pane = built.borrow_mut().take().expect("the pane was built");
        RealPane {
            _connection: connection,
            cx,
            fake,
            window: window.into(),
            pane,
        }
    }

    /// Run the header's `action` as a press on its button would, with a fresh input mark.
    fn header_action(&mut self, action: Action) {
        let pane = self.pane.clone();
        self.cx
            .update_window(self.window, |_, window, cx| {
                let mark = Mark::take(cx);
                pane.update(cx, |pane, cx| pane.header_action(action, &mark, window, cx))
                    .expect("the pane is alive");
            })
            .unwrap();
    }

    /// Press Tab: dispatch the action Tab is bound to from wherever focus is.
    fn tab(&mut self) {
        self.cx
            .update_window(self.window, |_, window, cx| {
                window.dispatch_action(Box::new(crate::actions::FocusNext), cx)
            })
            .unwrap();
    }

    /// Draw a few frames, letting what each asked for run, and return the driver record of the last one.
    fn draw(&mut self) -> Vec<gpui::DriverNode> {
        for _ in 0..3 {
            self.cx.run_until_parked();
            self.cx
                .update_window(self.window, |_, window, cx| {
                    window.set_driver_enabled(true);
                    window.draw(cx).clear(cx);
                })
                .unwrap();
        }
        self.cx
            .update_window(self.window, |_, window, _| window.driver_nodes().to_vec())
            .unwrap()
    }

    /// Make the window `width` pixels wide, as the platform reports a resize.
    fn resize(&mut self, width: f32) {
        self.cx
            .update_window(self.window, |_, window, cx| {
                window.resize(size(px(width), px(300.)));
                window.bounds_changed(cx);
            })
            .unwrap();
    }

    /// Answer what the app sent, in order: the open session's tunnel read lists one tunnel to `host`, and everything
    /// else succeeds with no body. Panics unless the read was sent, the premise of every pill these tests draw.
    fn open_tunnel(&mut self, host: &str) {
        self.open_tunnels(host, 1);
    }

    /// [`RealPane::open_tunnel`], with `count` tunnels listed: two or more draw the pill as a count rather than one
    /// tunnel's mapping.
    fn open_tunnels(&mut self, host: &str, count: u16) {
        let tunnels: Vec<Value> = (1..=count)
            .map(|n| json!({ "id": format!("t{n}"), "local_port": 54320 + n, "remote_port": 5172 + n }))
            .collect();
        let mut read = false;
        for command in self.fake.commands() {
            if command.method == Method::Get && command.path == "/api/sessions/a/tunnels" {
                read = true;
                self.fake.reply(
                    command.id,
                    200,
                    Some(json!({
                        "available": true,
                        "host_name": host,
                        "consent": true,
                        "tunnels": tunnels,
                    })),
                );
            } else {
                self.fake.reply(command.id, 200, None);
            }
        }
        assert!(
            read,
            "the premise: the remote session's terminal read its tunnels"
        );
    }
}

/// The node drawn with `id` in `nodes`; panics, naming what was drawn, when there is none or more than one, since each
/// control must be drawn once (the fit lays out a copy of the actions it never draws).
fn only_node<'a>(nodes: &'a [gpui::DriverNode], id: &str) -> &'a gpui::DriverNode {
    let found: Vec<_> = nodes
        .iter()
        .filter(|node| node.element_id.as_deref() == Some(id))
        .collect();
    match found.as_slice() {
        [node] => node,
        _ => panic!("{id} was drawn {} times", found.len()),
    }
}

/// A snapshot with an idle session `a` on the pinned host list's SSH host 2, so its terminal reads its tunnels as it
/// opens.
fn remote_session() -> Vec<CollectionState> {
    remote_session_skipping(false)
}

/// [`remote_session`], with **Skip session confirmations** set to `skip`, so the header's Delete can be sent unasked.
fn remote_session_skipping(skip: bool) -> Vec<CollectionState> {
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
    let mut snapshot = snapshot_of(vec![remote], skip);
    snapshot.push(CollectionState {
        collection: Collection::Hosts,
        meta: meta(&keys),
        entries: host_entries,
    });
    snapshot
}

/// A snapshot with an idle session `a` on a container environment's host (host 2, remote), whose header draws Suspend
/// left of Stop.
fn environment_session() -> Vec<CollectionState> {
    environment_session_with_tabs(&[])
}

/// [`environment_session`], its session listing the shell tabs `tabs` (by id, in the helm's order).
fn environment_session_with_tabs(tabs: &[&str]) -> Vec<CollectionState> {
    use crate::session_list::environment_tests::{Provider, connected, environment_host, registry};
    let mut row = entry(&idle("a"), 0);
    row.value["tabs"] = json!(
        tabs.iter()
            .map(|id| json!({ "id": id }))
            .collect::<Vec<_>>()
    );
    let mut snapshot = snapshot_of(vec![row], false);
    snapshot.push(registry(&environment_host(
        Provider::Container,
        connected(),
    )));
    snapshot
}

/// A tunnel host's name long enough that the pill would show it past the pill's widest (280 pixels).
fn long_host() -> String {
    format!("a-very-long-host-name{}.example.internal", "-x".repeat(40))
}

/// SPEC.md: "Text size, Stop, both parts of each split button, Clone, Delete and the tunnel pill remain fully visible
/// and keyboard reachable from a 650px main pane; … Narrower panes may clip the row, which never wraps." Before the
/// actions could condense, a pill at its floor (154 pixels, which keeps both ports readable) was drawn over the first
/// 49 pixels of the actions at 650 pixels; the old app condensed its actions instead (`header-layout.js` and
/// `.actions-compact`), and the terminal area agreed on 2026-11-16 that the pill keeps its floor. The tab strip's old
/// 112-pixel floor beside a pill competes for the same width, and at 650 pixels neither row has room for all of it:
/// kept fixed, it draws the pill over the first action, and dropped, it lets the pill take width from the strip that
/// the pill's own floor does not need, so it gives way only as far as the pill's floor needs (decided with the session
/// view's owner on 2026-11-16). Specified: in a 650-pixel pane, for a remote session and for a container environment's
/// session, once the terminal lists one tunnel to a very long host name, every action, both carets, the text size group
/// and the environment's control are drawn once each, whole, inside the pane, on one line, each but the readout
/// tracking a focus handle so Tab reaches it; the pill is drawn whole, no narrower than its floor and ending left of
/// the actions; the tab strip is narrower than 112 pixels and ends the row's 6-pixel gap left of the pill, which is
/// exactly its 154-pixel floor wide, so the strip has all the room the pill's floor leaves; Stop is narrower than it
/// was before the pill showed, by more than its side padding (the condensed look); three more frames draw Stop and the
/// pill in the same place, so the look does not flip between frames; and the same row widened to 1400 pixels draws
/// Stop as wide as before the pill showed, and narrowed to 650 again draws it condensed as before, so a condensed look
/// neither sticks nor fails to come back.
#[test]
fn a_pill_at_650_pixels_condenses_the_actions_beside_it() {
    use super::header::ENVIRONMENT_ID;
    use crate::terminal::set::{STRIP_FLOOR_BESIDE_PILL, STRIP_ID};
    use crate::terminal::text_size::SMALLER_ID;
    for (name, snapshot, environment) in [
        ("remote", remote_session(), false),
        ("environment", environment_session(), true),
    ] {
        let mut pane = RealPane::open(650., snapshot);
        let before = pane.draw();
        assert!(
            !before
                .iter()
                .any(|node| node.element_id.as_deref() == Some("session-tunnel-pill")),
            "the premise: no pill before the tunnel read is answered ({name})"
        );
        let ordinary_stop = only_node(&before, Action::Stop.id()).bounds;
        pane.open_tunnel(&long_host());
        let nodes = pane.draw();
        assert_whole_on_one_line(&nodes);
        let mut controls: Vec<&str> = Action::ALL.iter().map(|action| action.id()).collect();
        controls.extend(CARETS);
        if environment {
            controls.push(ENVIRONMENT_ID);
        }
        for id in &controls {
            let node = only_node(&nodes, id);
            assert!(node.focus.is_some(), "{id} tracks a focus handle ({name})");
        }
        if environment {
            let control = only_node(&nodes, ENVIRONMENT_ID);
            assert!(
                control.bounds.size.width > px(0.)
                    && control.visible_bounds == control.bounds
                    && control.bounds.origin.x + control.bounds.size.width <= px(650.),
                "the environment's control is whole inside the pane ({name}): {:?}",
                control.bounds
            );
        }
        let pill = only_node(&nodes, "session-tunnel-pill").bounds;
        let first = only_node(&nodes, SMALLER_ID).bounds;
        assert!(
            pill.size.width >= px(154.) && pill.origin.x + pill.size.width <= first.origin.x,
            "the pill keeps its floor and ends left of the actions ({name}): pill {pill:?}, first action {first:?}"
        );
        assert_eq!(
            only_node(&nodes, "session-tunnel-pill").visible_bounds,
            pill,
            "the pill is drawn whole ({name})"
        );
        // The strip's floor beside the pill: neither row has room for all 112 pixels of it beside the pill at its
        // floor and the condensed actions, so it gives way, but only that far: the strip has every pixel the pill's
        // floor leaves, the pill no more than its floor, with the row's 6-pixel gap between them (the module docs of
        // `header/fit.rs`).
        let strip = only_node(&nodes, STRIP_ID).bounds;
        assert!(
            strip.size.width < px(STRIP_FLOOR_BESIDE_PILL),
            "the premise: the strip gives up part of its floor ({name}): {strip:?}"
        );
        assert!(
            pill.size.width == px(154.)
                && strip.origin.x + strip.size.width + px(6.) == pill.origin.x,
            "the strip takes all the room the pill's floor leaves ({name}): strip {strip:?}, pill {pill:?}"
        );
        let stop = only_node(&nodes, Action::Stop.id()).bounds;
        assert!(
            stop.size.width + px(16.) < ordinary_stop.size.width,
            "Stop is condensed ({name}): {stop:?}, ordinary {ordinary_stop:?}"
        );
        for _ in 0..3 {
            let again = pane.draw();
            assert_eq!(
                (
                    only_node(&again, Action::Stop.id()).bounds,
                    only_node(&again, "session-tunnel-pill").bounds
                ),
                (stop, pill),
                "the next frame keeps the look ({name})"
            );
        }
        pane.resize(1400.);
        let wide = pane.draw();
        assert_eq!(
            only_node(&wide, Action::Stop.id()).bounds.size,
            ordinary_stop.size,
            "a wide pane brings the ordinary look back ({name})"
        );
        pane.resize(650.);
        let narrow = pane.draw();
        assert_eq!(
            only_node(&narrow, Action::Stop.id()).bounds,
            stop,
            "narrowed again, the row condenses again ({name})"
        );
    }
}

/// The old app condensed its actions only for measured pressure: "a wide row keeps its ordinary spacing even with
/// tunnels" (the old stylesheet's comment on `.actions-compact`). The same row is where the tab strip's floor beside a
/// pill shows on its own: the strip's tabs ask for less than 112 pixels, so only the floor (the old `min-width:
/// 112px`) makes it wider, and a floor that went missing or gave way with room to spare would show here. Specified: in
/// a 1400-pixel pane, once a remote session's terminal lists one tunnel to a very long host name, the pill is drawn and
/// Stop, Restart and the gap between them have the same widths as before the pill showed, and the tab strip is at
/// least 112 pixels wide and ends left of the pill.
#[test]
fn a_wide_pane_keeps_the_ordinary_actions_beside_a_pill() {
    use crate::terminal::set::{STRIP_FLOOR_BESIDE_PILL, STRIP_ID};
    let mut pane = RealPane::open(1400., remote_session());
    let before = pane.draw();
    let gap = |nodes: &[gpui::DriverNode]| {
        let stop = only_node(nodes, Action::Stop.id()).bounds;
        let restart = only_node(nodes, Action::Restart.id()).bounds;
        (
            stop.size.width,
            restart.size.width,
            restart.origin.x - (stop.origin.x + stop.size.width),
        )
    };
    let ordinary = gap(&before);
    let bare = only_node(&before, STRIP_ID).bounds;
    assert!(
        bare.size.width < px(STRIP_FLOOR_BESIDE_PILL),
        "the premise: without a pill the strip is narrower than its floor beside one: {bare:?}"
    );
    pane.open_tunnel(&long_host());
    let nodes = pane.draw();
    let pill = only_node(&nodes, "session-tunnel-pill").bounds;
    assert_eq!(
        gap(&nodes),
        ordinary,
        "the actions keep their ordinary look"
    );
    let strip = only_node(&nodes, STRIP_ID).bounds;
    assert!(
        strip.size.width >= px(STRIP_FLOOR_BESIDE_PILL)
            && strip.origin.x + strip.size.width <= pill.origin.x,
        "the strip keeps its floor beside the pill: strip {strip:?}, pill {pill:?}"
    );
}

/// SPEC.md keeps the tunnel pill fully visible down to a 650-pixel pane, and a pill counting two or more tunnels has
/// no floor of its own (unlike one showing a mapping, which shrinks to 154 pixels by cutting the host's name): any
/// width it gives up cuts its words. So beside such a pill the strip's floor must leave the pill its whole content
/// width, not its floor of nothing, or the strip's 112 pixels squeeze it (to 58 of its 100 pixels, measured), and the
/// pill must keep that width as a floor of its own, or it shrinks alongside the strip (to 88). Specified: for a
/// container environment's session whose terminal lists two tunnels, the pill drawn in a 1400-pixel pane (where
/// nothing presses on it) has the same width when the pane is narrowed to 650 pixels; there it is drawn whole and ends
/// left of the first action, and the strip ends the row's 6-pixel gap left of it.
#[test]
fn a_count_pill_at_650_pixels_keeps_its_whole_width() {
    use crate::terminal::set::STRIP_ID;
    use crate::terminal::text_size::SMALLER_ID;
    let mut pane = RealPane::open(1400., environment_session());
    pane.draw();
    pane.open_tunnels(&long_host(), 2);
    let wide = only_node(&pane.draw(), "session-tunnel-pill").bounds;
    pane.resize(650.);
    let nodes = pane.draw();
    let pill = only_node(&nodes, "session-tunnel-pill");
    let first = only_node(&nodes, SMALLER_ID).bounds;
    let strip = only_node(&nodes, STRIP_ID).bounds;
    assert!(
        pill.bounds.size.width < px(154.),
        "the premise: the pill is a count, narrower than a mapping's floor: {:?}",
        pill.bounds
    );
    assert_eq!(
        pill.bounds.size.width, wide.size.width,
        "the pill keeps its whole width at 650 pixels: {:?}, at 1400 {wide:?}",
        pill.bounds
    );
    assert!(
        pill.visible_bounds == pill.bounds && pill.bounds.right() <= first.origin.x,
        "the pill is drawn whole, left of the first action: {:?}, visible {:?}, first action {first:?}",
        pill.bounds,
        pill.visible_bounds
    );
    assert_eq!(
        strip.right() + px(6.),
        pill.bounds.origin.x,
        "the strip ends the gap left of the pill: strip {strip:?}"
    );
}

/// SPEC.md keeps every control of the top row keyboard reachable from a 650-pixel pane, and the tab strip is the part
/// of the row that scrolls rather than drops a control. Beside a pill at 650 pixels, an environment session's strip
/// gives up nearly all of its 112-pixel floor (`header/fit.rs`), leaving it narrower than a single tab, so its controls
/// stay usable only through that scrolling: one that took focus out of view would be invisible. Specified: in a
/// 650-pixel pane, for a container environment's session with one shell tab, once the terminal lists one tunnel to a
/// very long host name, the strip is narrower than its Agent tab alone; the Agent tab, the shell tab, its × and New
/// terminal are each drawn once and track a focus handle; pressing Tab from where focus starts reaches each of them
/// within 40 presses; and each of them, once focused, is scrolled into the strip far enough that part of it is visible.
/// The × is the case that needs its own rule: it sits at the end of an item wider than the strip, whose start is what
/// scrolling to the item would show (`TerminalSet::reveal_focused`).
#[test]
fn a_narrowed_strip_keeps_its_tabs_reachable_at_650_pixels() {
    use crate::terminal::set::{ADD_ID, AGENT_TAB_ID, STRIP_ID, close_element_id, tab_element_id};
    let mut pane = RealPane::open(650., environment_session_with_tabs(&["t1"]));
    pane.draw();
    pane.open_tunnel(&long_host());
    let nodes = pane.draw();
    only_node(&nodes, "session-tunnel-pill");
    let strip = only_node(&nodes, STRIP_ID).bounds;
    let agent = only_node(&nodes, AGENT_TAB_ID).bounds;
    assert!(
        strip.size.width < agent.size.width,
        "the premise: the strip is narrower than its Agent tab alone: strip {strip:?}, Agent {agent:?}"
    );
    let controls = [
        AGENT_TAB_ID.to_owned(),
        tab_element_id("t1"),
        close_element_id("t1"),
        ADD_ID.to_owned(),
    ];
    for id in &controls {
        assert!(
            only_node(&nodes, id).focus.is_some(),
            "{id} is drawn and tracks a focus handle"
        );
    }
    let mut reached: Vec<String> = Vec::new();
    for _ in 0..40 {
        pane.tab();
        let nodes = pane.draw();
        let Some(focused) = nodes
            .iter()
            .find(|node| node.flags.focused && node.element_id.is_some())
        else {
            continue;
        };
        let id = focused.element_id.as_deref().unwrap_or_default().to_owned();
        if controls.contains(&id) {
            let visible = focused.visible_bounds;
            assert!(
                visible.size.width > px(0.) && visible.size.height > px(0.),
                "{id}, focused, is scrolled into view: visible {visible:?}, bounds {:?}, strip {strip:?}",
                focused.bounds
            );
        }
        if controls.contains(&id) && !reached.contains(&id) {
            reached.push(id);
        }
        if reached.len() == controls.len() {
            break;
        }
    }
    assert_eq!(
        reached.len(),
        controls.len(),
        "Tab reaches each of the strip's controls; reached {reached:?}"
    );
}

/// The strip's floor beside a pill is the fit's to apply, and the row draws its left region without the fit when it
/// has no actions to fit: no store, no session row, or a Delete in flight, whose progress then shows in place of the
/// actions. Those rows were not part of the decision on the floor, so they keep the layout they had before it.
/// A running Delete is the one a user meets with a pill showing. Specified: in a 1400-pixel pane with **Skip session
/// confirmations** on, for a remote session, once the terminal lists one tunnel to a very long host name the strip is
/// at least 112 pixels wide (the floor applies while the actions show); once the header's Delete is sent and left
/// unanswered, the Delete's progress and the pill are drawn, no action is, and the strip has exactly the bounds it had
/// before the pill showed (no floor).
#[test]
fn a_running_delete_draws_the_strip_beside_a_pill_without_its_floor() {
    use crate::terminal::set::{STRIP_FLOOR_BESIDE_PILL, STRIP_ID};
    let mut pane = RealPane::open(1400., remote_session_skipping(true));
    let bare = only_node(&pane.draw(), STRIP_ID).bounds;
    pane.open_tunnel(&long_host());
    let floored = only_node(&pane.draw(), STRIP_ID).bounds;
    assert!(
        floored.size.width >= px(STRIP_FLOOR_BESIDE_PILL) && bare.size.width < floored.size.width,
        "the premise: beside the pill and the actions the strip has its floor: {floored:?}, without the pill {bare:?}"
    );
    pane.header_action(Action::Delete);
    let nodes = pane.draw();
    assert!(
        pane.fake
            .commands()
            .iter()
            .any(|command| command.method == Method::Delete
                && command.path.starts_with("/api/sessions/a")),
        "the premise: the Delete was sent unasked and is unanswered"
    );
    only_node(&nodes, "session-header-delete-progress-a");
    only_node(&nodes, "session-tunnel-pill");
    for action in Action::ALL {
        assert!(
            !nodes
                .iter()
                .any(|node| node.element_id.as_deref() == Some(action.id())),
            "{action:?} gave way to the progress"
        );
    }
    assert_eq!(
        only_node(&nodes, STRIP_ID).bounds,
        bare,
        "the strip is laid out as it was before the pill showed"
    );
}

/// On a row with no actions to fit, neither the strip nor the pill is wrapped in a floor box, so the pill keeps its own
/// floor and the strip, which scrolls, gives way, as before floors existed. A box with no floor shrank alongside the
/// strip and let the pill spill out of it over the Delete's progress.
///
/// Spec: in a 650-pixel pane with **Skip session confirmations** on, for a remote session listing fifteen shell tabs,
/// once the terminal lists one tunnel to a very long host name and the header's Delete is sent and left unanswered, the
/// pill is at least 154 pixels wide and ends left of the Delete's progress.
#[test]
fn a_running_delete_in_a_narrow_pane_keeps_the_pill_whole_beside_many_tabs() {
    let mut snapshot = remote_session_skipping(true);
    let tabs: Vec<Value> = (1..=15).map(|n| json!({ "id": format!("t{n}") })).collect();
    let row = snapshot
        .iter_mut()
        .flat_map(|collection| collection.entries.iter_mut())
        .find(|entry| entry.value.get("host").is_some() && entry.value.get("tabs").is_some())
        .expect("the premise: the snapshot holds session a's row");
    row.value["tabs"] = json!(tabs);
    let mut pane = RealPane::open(650., snapshot);
    pane.open_tunnel(&long_host());
    pane.draw();
    pane.header_action(Action::Delete);
    let nodes = pane.draw();
    let progress = only_node(&nodes, "session-header-delete-progress-a").bounds;
    let pill = only_node(&nodes, "session-tunnel-pill").bounds;
    assert!(
        pill.size.width >= px(154.) && pill.origin.x + pill.size.width <= progress.origin.x,
        "the pill keeps its floor and ends left of the progress: pill {pill:?}, progress {progress:?}"
    );
}

/// SPEC.md's 650-pixel rule holds for an environment session too, whose header carries one more control, Suspend or
/// Resume, left of Stop, and whose pending states once widened that control until Delete was cut off (the review of
/// 2026-11-16 measured Delete 52 of 72 pixels visible while suspending). Specified: in a 650-pixel pane with an idle
/// session on a container host, the environment control, each of the five actions and both carets are drawn whole
/// inside the pane on one line, and the control's width is the same, while the host is connected (Suspend), suspending,
/// suspended (Resume) and resuming.
#[test]
fn an_environment_sessions_actions_stay_whole_in_a_650_pixel_pane() {
    use super::header::ENVIRONMENT_ID;
    use crate::session_list::environment_tests::{
        Provider, changing, connected, environment_host, registry, suspended_at,
    };
    use farhelm_proto::tensorlake::LifecycleDirection;
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        assets::UI_FONT,
    ));
    let mut cx =
        HeadlessAppContext::with_platform(text_system, Arc::new(assets::Assets), || Ok(None));
    cx.update(bootstrap::install).unwrap();
    let (mut fake, handle, events) = FakeHelm::new();
    let _connection = cx.update(|cx| crate::session::start(handle, events, None, cx));
    cx.run_until_parked();
    let mut snapshot = snapshot_of(vec![entry(&idle("a"), 0)], false);
    snapshot.push(registry(&environment_host(
        Provider::Container,
        connected(),
    )));
    fake.connect(snapshot);
    cx.run_until_parked();
    let window = cx
        .open_window(size(px(650.), px(300.)), |window, cx| {
            let view = cx.new(|cx| {
                let list = cx.new(SessionList::new);
                let new = list.read(cx).new_focus().clone();
                let pane = cx.new(|cx| SessionPane::new(new, window, cx));
                PaneOnly {
                    pane,
                    _list: list,
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
    let mut widths = Vec::new();
    for (name, state) in [
        ("connected", connected()),
        ("suspending", changing(LifecycleDirection::Suspending)),
        ("suspended", suspended_at(0)),
        ("resuming", changing(LifecycleDirection::Resuming)),
    ] {
        let host = environment_host(Provider::Container, state);
        fake.push(CollectionChanges {
            collection: Collection::Hosts,
            meta: registry(&host).meta,
            upsert: vec![registry(&host).entries.remove(1)],
            remove: Vec::new(),
        });
        for command in fake.commands() {
            fake.reply(command.id, 200, None);
        }
        let nodes = draw(&mut cx);
        assert_whole_on_one_line(&nodes);
        let control = nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(ENVIRONMENT_ID))
            .unwrap_or_else(|| panic!("the environment control is drawn ({name})"));
        let stop = drawn_node(&nodes, Action::Stop).bounds;
        let bounds = control.bounds;
        assert!(
            bounds.size.width > px(0.)
                && control.visible_bounds == bounds
                && bounds.origin.x >= px(0.)
                && bounds.origin.x + bounds.size.width <= stop.origin.x,
            "the control is whole and left of Stop ({name}): {bounds:?}, visible {:?}",
            control.visible_bounds
        );
        assert_eq!(
            bounds.origin.y, stop.origin.y,
            "on the actions' line ({name})"
        );
        widths.push((name, bounds.size.width));
    }
    let (_, suspend) = widths[0];
    let (_, resume) = widths[2];
    assert_eq!(
        widths[1].1, suspend,
        "suspending keeps Suspend's width: {widths:?}"
    );
    assert_eq!(
        widths[3].1, resume,
        "resuming keeps Resume's width: {widths:?}"
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

/// A refused or failed write must not leave the terminals at a size the helm never stored, which the next start would
/// disagree with: the hold ends with the helm's answer whatever it is, and the size falls back to the stored one.
/// Specified: at a stored 16, Larger shows 17; the helm's refusal brings back 16 and nothing more is sent.
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

/// The text size group sits in the header's row of buttons, and a group taller than its neighbors reads as a different
/// control: the port's first group was 30 pixels against the buttons' 23, and the old app's was shorter than its
/// buttons, which the maintainer counted as a bug too (decided on 2026-11-16 that it matches them). The group's height
/// comes from `Button::compact_height`, so this also checks that figure against the buttons it describes.
///
/// Spec: in a 1400-pixel pane for a remote session with no tunnel pill (the header's ordinary look), and in a 650-pixel
/// pane once that session's terminal lists one tunnel to a very long host name (the condensed look), Stop is drawn at
/// `Button::compact_height` for its look (ordinary, or `Condensed::Words` when condensed, 2 pixels shorter), and the
/// text size group's outer box has Stop's top and bottom, so it is exactly as tall as Stop and level with it.
#[test]
fn the_text_size_group_is_as_tall_as_the_header_buttons_in_both_looks() {
    use crate::components::{Button, Condensed};
    use crate::terminal::text_size::GROUP_ID;
    let check = |look: &str, nodes: &[gpui::DriverNode], condensed: Option<Condensed>| {
        let stop = only_node(nodes, Action::Stop.id()).bounds;
        assert_eq!(
            stop.size.height,
            Button::compact_height(condensed),
            "the premise: Stop is drawn in the {look} look"
        );
        let group = only_node(nodes, GROUP_ID).bounds;
        assert_eq!(
            (group.top(), group.bottom()),
            (stop.top(), stop.bottom()),
            "the {look} text size group spans Stop's height: group {group:?}, Stop {stop:?}"
        );
    };

    let mut wide = RealPane::open(1400., remote_session());
    let nodes = wide.draw();
    assert!(
        !nodes
            .iter()
            .any(|node| node.element_id.as_deref() == Some("session-tunnel-pill")),
        "the premise: no pill in the ordinary look"
    );
    check("ordinary", &nodes, None);

    let mut narrow = RealPane::open(650., remote_session());
    narrow.draw();
    narrow.open_tunnel(&long_host());
    let nodes = narrow.draw();
    only_node(&nodes, "session-tunnel-pill");
    check("condensed", &nodes, Some(Condensed::Words));
}

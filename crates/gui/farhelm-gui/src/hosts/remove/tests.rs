//! Tests of removing a host from its menu over the helm client's fake: the confirmation (what it shows, its answers'
//! order, where focus starts and returns), what each answer sends and under which key, removal without asking, where a
//! refused removal shows, and the two refusals before anything is sent (a session launch in flight, the host busy).
//!
//! The Hosts panel runs over the reference screens' three hosts (the local host as 1, `build-box` as 2 at
//! `dev@build-box`, `gpu-1` as 3), in a sidebar-wide column so a menu opens beside its row, and Remove is reached by
//! clicking a row's toggle and then the menu's item. Every step ends in one of the shared harness's step helpers, so
//! the focus and layer invariants are checked after each.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, HostView, KeyedEntry, PREFERENCES_KEY,
};
use gpui::{DriverNode, TestAppContext, VisualTestContext, px, size};
use serde_json::{Value, json};

use crate::focus::Mark;
use crate::hosts::tests::{hosts, open_panel_in_column, reference_hosts};
use crate::operations::{Command, NotStarted, OpKey, Work};
use crate::session;
use crate::settings::Refused;
use crate::test_support::{any_open, click, focused_id, frame, keys, meta, settle};

/// The helm's refusal of a host whose setup, update or uninstall is running, in its own words.
const BUSY: &str =
    "host 2 is busy with a setup, update or uninstall; remove it after that finishes";

/// The preference row with "Remove hosts without asking" set to `on`.
fn preferences(on: bool) -> CollectionState {
    CollectionState {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: json!({ "skip_host_remove_confirmation": on }),
        }],
    }
}

/// The panel over the reference hosts and `more` collections, in a column of the sidebar's width in a window as large
/// as the reference screens'.
fn open_hosts(
    cx: &mut TestAppContext,
    more: Vec<CollectionState>,
) -> (FakeHelm, &mut VisualTestContext) {
    let mut snapshot = vec![hosts(&reference_hosts())];
    snapshot.extend(more);
    let (helm, _, cx) = open_panel_in_column(cx, snapshot);
    cx.simulate_resize(size(px(1600.), px(1000.)));
    settle(cx);
    (helm, cx)
}

/// Open `build-box`'s menu from its row's toggle and choose Remove.
fn press_remove(cx: &mut VisualTestContext) {
    click(cx, "host-menu-toggle-2");
    assert!(
        node(&frame(cx), "host-menu-remove").is_some(),
        "the premise: Remove is offered"
    );
    click(cx, "host-menu-remove");
}

/// The node drawn under `id`, if any.
fn node(nodes: &[DriverNode], id: &str) -> Option<DriverNode> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .cloned()
}

/// The accessible name of what was drawn under `id`, if anything was drawn there with one.
fn label(cx: &mut VisualTestContext, id: &str) -> Option<String> {
    node(&frame(cx), id).and_then(|node| node.label.map(|label| label.to_string()))
}

/// Every line of text the window painted, joined with single spaces.
fn painted(cx: &mut VisualTestContext) -> String {
    let lines: Vec<String> = frame(cx)
        .into_iter()
        .flat_map(|node| node.texts.into_iter().map(|line| line.text.to_string()))
        .collect();
    lines
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Every command sent since the last call, as "<method> <path>" with its body, sorted so an assertion does not depend
/// on which of two requests an answer sends first.
fn sent(helm: &mut FakeHelm) -> Vec<(String, Option<Value>)> {
    let mut sent: Vec<(String, Option<Value>)> = helm
        .commands()
        .into_iter()
        .map(|command| {
            (
                format!("{:?} {}", command.method, command.path),
                command.body,
            )
        })
        .collect();
    sent.sort_by(|a, b| a.0.cmp(&b.0));
    sent
}

/// The work in flight on `build-box`'s key.
fn host_work(cx: &mut VisualTestContext) -> Option<Work> {
    cx.update(|_, cx| {
        session::current_operations(cx)?
            .read(cx)
            .in_flight(&OpKey::host("2"))
    })
}

/// Start `work` on `key` in the operation store, as the feature that owns it would, and return the command's id.
fn start_work(helm: &mut FakeHelm, cx: &mut VisualTestContext, key: OpKey, work: Work) -> u64 {
    cx.update(|_, cx| {
        let operations = session::current_operations(cx).expect("a UI session");
        let command = Command::new(
            work,
            Method::Post,
            ApiPath::api(&["work"]).unwrap(),
            None,
            "POST /api/work",
        );
        operations.update(cx, |operations, cx| {
            operations.start(&key, command, cx, |_, _| {}).unwrap();
        });
    });
    settle(cx);
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "the premise: the work went out");
    sent[0].id
}

/// Push the hosts list without `build-box`, as the helm does once it has forgotten it.
fn push_removed(helm: &mut FakeHelm, cx: &mut VisualTestContext) {
    let listed: Vec<HostView> = reference_hosts()
        .into_iter()
        .filter(|host| host.id != 2)
        .collect();
    let state = hosts(&listed);
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: Vec::new(),
        remove: vec![HostView::key(2)],
    });
    settle(cx);
}

// ---- The confirmation ----------------------------------------------------------------------------------------------

/// SPEC.md, "Topology": removing a host "merely forgets it — the supervisor and its sessions are untouched". The
/// confirmation is where the user reads that before anything changes, so it must name the host, show its full
/// destination (the title may shorten the name) and the old explanation, and offer the old three answers in their order,
/// starting on Remove, the action the user asked for (SPEC.md, "Opening and closing", lists host removal). Specified:
/// the menu's Remove opens "Remove host · build-box", summarized "Connected · Farhelm 0.0.0-unreleased", with
/// `dev@build-box`, the explanation and the footer note naming the gear; the answers read Cancel, "Remove, don't ask
/// again" and Remove in that order; focus is on Remove; nothing was sent.
#[gpui::test]
fn remove_asks_naming_the_host_its_destination_and_what_removal_keeps(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx, vec![preferences(false)]);
    press_remove(cx);
    assert_eq!(
        label(cx, "prompt").as_deref(),
        Some("Remove host · build-box")
    );
    let text = painted(cx);
    for shown in [
        "Connected · Farhelm 0.0.0-unreleased",
        "dev@build-box",
        "Farhelm will forget this host. Its supervisor and sessions keep running, and adding the destination again \
         finds them.",
        "Undo \"don't ask again\" with the gear at the foot of the navigation rail",
    ] {
        assert!(text.contains(shown), "{shown:?} is drawn: {text}");
    }
    let answers: Vec<String> = frame(cx)
        .into_iter()
        .filter(|node| {
            node.element_id.as_deref().is_some_and(|id| {
                matches!(
                    id,
                    "prompt-cancel" | "prompt-alternative-0" | "prompt-confirm"
                )
            })
        })
        .filter_map(|node| node.label.map(|label| label.to_string()))
        .collect();
    assert_eq!(answers, ["Cancel", "Remove, don't ask again", "Remove"]);
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    assert_eq!(sent(&mut helm), [], "nothing was sent before an answer");
}

/// Cancel keeps the host, and closing a question the user opened returns focus to where it was before (SPEC.md,
/// "Opening and closing"): the row's menu toggle, on every platform. On macOS a click leaves the toggle unfocused, so
/// this runs under that click rule, where only the menu's naming of its toggle puts focus there. Specified: under
/// macOS's click rule, Cancel closes the question with nothing sent and focus on `build-box`'s toggle; Escape does the
/// same after a second Remove.
#[gpui::test]
fn cancel_sends_nothing_and_returns_focus_to_the_rows_toggle(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx, vec![preferences(false)]);
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    settle(cx);
    press_remove(cx);
    assert!(any_open(cx), "the premise: the question opened");
    click(cx, "prompt-cancel");
    assert!(!any_open(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-toggle-2"));

    press_remove(cx);
    keys(cx, "escape");
    assert!(!any_open(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-toggle-2"));
    assert_eq!(sent(&mut helm), []);
}

/// The question is frozen on the host it named, so it must go when that host does rather than linger over a row that is
/// gone (SPEC.md, "Keyboard focus": a dialog goes "when the thing it is about is gone"). Specified: with the question
/// open, a pushed hosts list without `build-box` closes it, and nothing is sent.
#[gpui::test]
fn the_question_closes_when_its_host_is_gone(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx, vec![preferences(false)]);
    press_remove(cx);
    assert!(any_open(cx), "the premise: the question opened");
    push_removed(&mut helm, cx);
    assert!(!any_open(cx));
    assert_eq!(sent(&mut helm), []);
}

// ---- The answers ---------------------------------------------------------------------------------------------------

/// Remove sends the helm's own route for forgetting a host, naming it by registry id, under the host's key, so the
/// request goes on after the question closes and the host's other actions wait for it; once the helm forgets the host,
/// its row goes, and nothing opens. Specified: Remove closes the question with focus on `build-box`'s toggle and sends
/// `DELETE /api/hosts/2` without a body as the host key's `RemoveHost` work; the helm's empty reply frees the key, and
/// after the pushed list drops the host its row is gone and nothing is open. (Where focus goes once the toggle is gone
/// is the focus coordinator's recovery; the harness checks after every step that something holds it.)
#[gpui::test]
fn remove_sends_the_delete_under_the_hosts_key(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx, vec![preferences(false)]);
    press_remove(cx);
    click(cx, "prompt-confirm");
    assert!(!any_open(cx), "the question closed at once");
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-toggle-2"));
    let commands = helm.commands();
    assert_eq!(commands.len(), 1, "one command: {commands:?}");
    assert_eq!(
        (commands[0].method, commands[0].path.as_str()),
        (Method::Delete, "/api/hosts/2")
    );
    assert_eq!(commands[0].body, None);
    assert_eq!(host_work(cx), Some(Work::RemoveHost));

    push_removed(&mut helm, cx);
    helm.reply(commands[0].id, 200, Some(json!({})));
    settle(cx);
    assert_eq!(host_work(cx), None, "the reply frees the key");
    assert!(node(&frame(cx), "host-row-2").is_none(), "the row is gone");
    assert!(!any_open(cx), "and nothing opened");
}

/// A refused removal leaves the host listed, so its row is where the refusal belongs, and the row opens to show it
/// (the old dialog showed it inside itself; the question has closed by now). Specified: the helm's 409 with its words
/// shows under `build-box`'s row, and nothing opens.
#[gpui::test]
fn a_refused_removal_shows_under_the_hosts_row(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx, vec![preferences(false)]);
    press_remove(cx);
    click(cx, "prompt-confirm");
    let commands = helm.commands();
    assert_eq!(commands.len(), 1, "the premise: the removal went out");
    helm.reply(commands[0].id, 409, Some(json!(BUSY)));
    settle(cx);
    assert_eq!(label(cx, "host-error-2").as_deref(), Some(BUSY));
    assert!(!any_open(cx));
}

/// "Remove, don't ask again" is the old app's permanent answer: it removes this host and turns on the shared preference
/// that skips the question next time, which Settings shows and can turn off (`settings.spec.ts`, "settings reverses a
/// permanent removal answer without reloading"). Specified: it closes the question and sends `DELETE /api/hosts/2` and
/// `PUT /api/preferences` with only `{"skip_host_remove_confirmation": true}`, in either order.
#[gpui::test]
fn the_permanent_answer_also_stops_asking(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx, vec![preferences(false)]);
    press_remove(cx);
    click(cx, "prompt-alternative-0");
    assert!(!any_open(cx));
    assert_eq!(
        sent(&mut helm),
        [
            (format!("{:?} /api/hosts/2", Method::Delete), None),
            (
                format!("{:?} /api/preferences", Method::Put),
                Some(json!({ "skip_host_remove_confirmation": true }))
            ),
        ]
    );
}

/// The permanent answer's preference save is separate from the removal, and the question has closed by the time it is
/// refused, so the refusal must show somewhere or the user believes the next removal will not ask. Specified: a 500 to
/// the preference save shows "Remove hosts without asking:" and the helm's words on the Hosts panel's line; it goes
/// once the pushed preference is on.
#[gpui::test]
fn a_refused_permanent_answer_shows_on_the_panels_line(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx, vec![preferences(false)]);
    press_remove(cx);
    click(cx, "prompt-alternative-0");
    let commands = helm.commands();
    let removal = commands
        .iter()
        .find(|command| command.path == "/api/hosts/2")
        .expect("the removal went out");
    let save = commands
        .iter()
        .find(|command| command.path == "/api/preferences")
        .expect("the preference save went out");
    // The fake answers in the order the commands were sent: the removal first, which goes ahead whatever the save says.
    helm.reply(removal.id, 200, Some(json!({})));
    helm.reply(save.id, 500, Some(json!("the helm's database refused")));
    settle(cx);
    assert_eq!(
        label(cx, "hosts-remove-stop-asking-error").as_deref(),
        Some("Remove hosts without asking: the helm's database refused")
    );

    let on = preferences(true);
    helm.push(CollectionChanges {
        collection: on.collection,
        meta: on.meta,
        upsert: on.entries,
        remove: Vec::new(),
    });
    settle(cx);
    assert_eq!(label(cx, "hosts-remove-stop-asking-error"), None);
}

/// With "Remove hosts without asking" on, the menu's Remove goes ahead at once, as the old remembered answer did, and
/// focus stays on the row's toggle, where the closing menu returned it. Specified: with the preference on, Remove opens
/// no question and sends `DELETE /api/hosts/2` as the host key's work, focus on `build-box`'s toggle.
#[gpui::test]
fn with_the_preference_on_remove_goes_without_asking(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx, vec![preferences(true)]);
    press_remove(cx);
    assert!(!any_open(cx), "no question, and the menu closed");
    assert_eq!(
        sent(&mut helm),
        [(format!("{:?} /api/hosts/2", Method::Delete), None)]
    );
    assert_eq!(host_work(cx), Some(Work::RemoveHost));
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-toggle-2"));
}

// ---- Refused before anything is sent -------------------------------------------------------------------------------

/// What the Hosts panel's line says when a session launch held a Remove back.
const LAUNCH_REFUSAL: &str =
    "Remove build-box: not started, a session launch is in progress; try again when it finishes";

/// The migration's rule (decided 2026-11-14, after the old app refused a host change during a launch): a host's removal
/// is refused, with a reason, while a session launch is in flight, and the launcher refuses its side while a host
/// change is. The launcher's New or Clone creates a session, and so do a Restart and a Replace, so each counts. A press
/// that does nothing would leave the user guessing. Specified: while a launch under a fresh launch key, then a
/// session's Restart, then a session's Replace is in flight, Remove opens no question, sends nothing, and the Hosts
/// panel's line reads [`LAUNCH_REFUSAL`]; once that work is answered, Remove asks as usual and the line is gone.
#[gpui::test]
fn a_session_launch_in_flight_refuses_remove_with_a_reason(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx, vec![preferences(false)]);
    for (key, work) in [
        (OpKey::new_launch(), Work::Launch),
        (OpKey::session("s1"), Work::Restart),
        (OpKey::session("s1"), Work::Replace),
    ] {
        let launch = start_work(&mut helm, cx, key, work);
        press_remove(cx);
        assert!(!any_open(cx), "no question while a {work:?} runs");
        assert_eq!(sent(&mut helm), []);
        assert_eq!(
            label(cx, "hosts-remove-not-started").as_deref(),
            Some(LAUNCH_REFUSAL),
            "{work:?}"
        );

        helm.reply(launch, 200, None);
        settle(cx);
        press_remove(cx);
        assert_eq!(
            label(cx, "prompt").as_deref(),
            Some("Remove host · build-box")
        );
        assert_eq!(label(cx, "hosts-remove-not-started"), None);
        keys(cx, "escape");
    }
}

/// The launch check holds when the removal is answered, not only when Remove is pressed in the menu: a launch can start
/// while the question is open (from another surface, or an earlier press already on its way). Specified: with the
/// question open, a session's Restart starting makes Remove on the question send nothing, and the Hosts panel's line
/// reads [`LAUNCH_REFUSAL`].
#[gpui::test]
fn a_launch_that_started_while_the_question_was_open_refuses_its_answer(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx, vec![preferences(false)]);
    press_remove(cx);
    assert!(any_open(cx), "the premise: the question opened");
    start_work(&mut helm, cx, OpKey::session("s1"), Work::Restart);
    click(cx, "prompt-confirm");
    assert_eq!(sent(&mut helm), [], "no removal went out");
    assert_eq!(
        label(cx, "hosts-remove-not-started").as_deref(),
        Some(LAUNCH_REFUSAL)
    );
}

/// When the host turned busy between the menu and the question, the old dialog disabled Remove and its permanent
/// answer together; offering "Remove, don't ask again" then would leave a second way to remove a host the question
/// says cannot be removed now. Specified: asked while a save is in flight on `build-box`'s key, the question offers
/// Cancel and an unavailable Remove, no permanent answer and no footer note naming it, with focus on Cancel; pressing
/// Remove sends nothing and leaves it open, and Cancel closes it.
#[gpui::test]
fn asked_while_the_host_is_busy_only_cancel_acts(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx, vec![preferences(false)]);
    start_work(&mut helm, cx, OpKey::host("2"), Work::Save);
    cx.update(|window, cx| {
        let mark = Mark::take(cx);
        assert!(
            super::ask(2, &mark, window, cx).is_some(),
            "the question opened"
        );
    });
    settle(cx);
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));
    assert!(
        node(&frame(cx), "prompt-alternative-0").is_none(),
        "no permanent answer"
    );
    assert!(
        !painted(cx).contains("don't ask again"),
        "no footer note naming the permanent answer"
    );
    click(cx, "prompt-confirm");
    assert!(any_open(cx), "the unavailable Remove did nothing");
    assert_eq!(sent(&mut helm), []);
    click(cx, "prompt-cancel");
    assert!(!any_open(cx));
    assert_eq!(sent(&mut helm), []);
}

/// "Remove, don't ask again" answers one removal; when that removal is refused before anything is sent, the user's
/// "don't ask again" must not be stored either, or the next Remove would go ahead silently on a choice made for a
/// removal that never happened (the old app stored it first, a recorded deviation). Specified: with the question open,
/// a session's Restart starting makes the permanent answer send neither the removal nor the preference save, and the
/// Hosts panel's line gives the launch reason, [`LAUNCH_REFUSAL`].
#[gpui::test]
fn a_permanent_answer_whose_removal_is_refused_stores_no_preference(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx, vec![preferences(false)]);
    press_remove(cx);
    assert!(any_open(cx), "the premise: the question opened");
    start_work(&mut helm, cx, OpKey::session("s1"), Work::Restart);
    click(cx, "prompt-alternative-0");
    assert_eq!(
        sent(&mut helm),
        [],
        "neither the removal nor the preference save"
    );
    assert_eq!(
        label(cx, "hosts-remove-not-started").as_deref(),
        Some(LAUNCH_REFUSAL)
    );
}

/// The permanent answer's preference save shares the preference row's key with Settings' switches, the template
/// editor, ntfy's form, the checkout field and the setup confirmation, whose saves outlive their views, so one can be in
/// flight when the answer is pressed; the operation store then refuses this save, and the area's rule is that a failed
/// save is reported, never treated as saved. Specified: with a preference save in flight, "Remove, don't ask again"
/// sends the removal and no preference save, and the Hosts panel's line reads "Remove hosts without asking: " and
/// Settings' words for such a press ([`Refused::WORDS`], "Not saved: another operation was in progress. Try again.").
#[gpui::test]
fn a_permanent_answer_refused_by_another_preference_save_says_so(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx, vec![preferences(false)]);
    press_remove(cx);
    start_work(&mut helm, cx, OpKey::preferences(), Work::Save);
    click(cx, "prompt-alternative-0");
    assert_eq!(
        sent(&mut helm),
        [(format!("{:?} /api/hosts/2", Method::Delete), None)]
    );
    assert_eq!(
        label(cx, "hosts-remove-stop-asking-error"),
        Some(format!("Remove hosts without asking: {}", Refused::WORDS))
    );
}

/// The menu holds Remove unavailable while the host has work in flight, but work can start on the host after the
/// question opened; the operation store then refuses the removal, and that refusal must say so rather than do nothing.
/// Specified: with the question open, a save starting on `build-box`'s key makes Remove send nothing, and the Hosts
/// panel's line reads "Remove build-box: Not started: " and the operation store's shared reason
/// ([`NotStarted::BUSY_REASON`], "another operation was in progress. Try again.").
#[gpui::test]
fn a_busy_host_refuses_remove_with_a_reason(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx, vec![preferences(false)]);
    press_remove(cx);
    assert!(any_open(cx), "the premise: the question opened");
    start_work(&mut helm, cx, OpKey::host("2"), Work::Save);
    click(cx, "prompt-confirm");
    assert_eq!(sent(&mut helm), [], "no removal went out");
    assert_eq!(
        label(cx, "hosts-remove-not-started"),
        Some(format!(
            "Remove build-box: Not started: {}",
            NotStarted::BUSY_REASON
        ))
    );
}

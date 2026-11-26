//! Tests of the recovery card (`card.rs`): when the pane shows it in place of the open session's terminals and mounts
//! none, how it gives way to a new terminal once the session recovers, its Restart and inline Replace, a
//! legacy session's Replace with, the lines it takes over from the header, and the card as where focus rests.
//!
//! Everything runs in the real main view over the helm client's fake, whose connection hands out fake terminal streams
//! and records them (`test_support::open_main`), and reads what the window sent from the fake and what it drew from
//! GPUI's driver record of the frame. The card is `session-interrupted-card`; its buttons are `session-card-restart`
//! and `session-card-replace` (`session-card-replace-with` for a legacy launch); its wait line is
//! `session-card-<restart|replace>-progress-label-<key>`, and the refusal it draws is the header's line,
//! `session-header-outcome-<key>`. The first session of every snapshot is the open one.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::terminal::FakePhase;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::SessionStatus;
use farhelm_proto::ui_channel::SessionRow;
use gpui::TestAppContext;
use serde_json::{Value, json};

use crate::actions::{DELETE_SESSION_CHORD, FocusNextControl};
use crate::launcher::draft::{LaunchDraft, Mode, Tab};
use crate::launcher::launch_tests::{HostChange, creates_answering, start_host_change};
use crate::launcher::view_tests::fixture_hosts;
use crate::session_list::host_rail::tests::panel_shown;
use crate::session_list::lifecycle_tests::{consented, drawn_inside, refuse_for_yolo, replace};
use crate::session_list::menu;
use crate::session_list::menu_tests::{any_layer, drawn, label_of, push, snapshot};
use crate::session_list::outcome::PROGRESS_SHOWS_AFTER;
use crate::session_list::tests::{
    Fixture, change, launcher_drawn, listing_meta, painted, selected, session,
};
use crate::terminal::set::STRIP_ID;
use crate::test_support::{self, MainWindow, focused_id, frame, open_main};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The card's element id.
const CARD: &str = "session-interrupted-card";

/// `key` as an interrupted session: a host reboot ended its agent. The fixture's offer is Resume.
fn interrupted(key: &'static str) -> Fixture {
    session(key).status(json!({"state": "interrupted"}))
}

/// An exited row whose agent terminal is absent, with a separate shell tab still listed by the server.
fn missing_terminal(key: &'static str) -> Fixture {
    session(key)
        .status(json!({"state": "exited", "exit_code": 0}))
        .terminal_missing()
        .tabs(1)
}

/// The lifecycle requests the window sent since the last call, as (id, path, body), left unanswered; every other
/// command (a preference write, a seen mark) is answered with success at once.
fn lifecycle(fake: &mut FakeHelm) -> Vec<(u64, String, Value)> {
    let mut found = Vec::new();
    for FakeCommand {
        id,
        method,
        path,
        body,
        ..
    } in fake.commands()
    {
        let lifecycle = method == Method::Delete
            || (method == Method::Post
                && (path.ends_with("/restart") || path.ends_with("/replace")));
        if lifecycle {
            found.push((id, path, body.unwrap_or_default()));
        } else {
            fake.reply(id, 200, None);
        }
    }
    found
}

/// [`lifecycle`]'s requests as (path, body), for a test about what was sent.
fn sent(fake: &mut FakeHelm) -> Vec<(String, Value)> {
    lifecycle(fake)
        .into_iter()
        .map(|(_, path, body)| (path, body))
        .collect()
}

/// A plain, unconfirmed Restart of `key`, as the window sends it: nothing runs, so it carries no consent to stop one.
fn restart(key: &str) -> (String, Value) {
    (
        format!("/api/sessions/{key}/restart"),
        json!({"conversation": null, "stop_if_running": false, "with": null, "with_command": null,
               "confirm_yolo": false}),
    )
}

/// How many terminal streams the pane has opened so far, and the phase of each.
fn streams(main: &MainWindow) -> Vec<FakePhase> {
    main.terminals
        .borrow_mut()
        .iter_mut()
        .map(|terminal| terminal.phase())
        .collect()
}

/// The hover help shown once the pointer has rested on `selector` for the hover delay.
fn hover(cx: &mut gpui::VisualTestContext, selector: &'static str) -> Option<String> {
    let at = test_support::center(cx, selector);
    cx.simulate_mouse_move(at, None, gpui::Modifiers::none());
    test_support::advance(cx, crate::layers::HOVER_DELAY);
    cx.update(|window, cx| crate::layers::shown_tooltip(window, cx))
        .map(|shown| shown.text.to_string())
}

/// The status the current UI session's store lists the session filed under `key` with.
fn listed_status(cx: &mut gpui::VisualTestContext, key: &str) -> Option<SessionStatus> {
    cx.update(|_, cx| {
        crate::session::current_store(cx)?
            .read(cx)
            .collections()
            .get::<SessionRow>(key)
            .map(|row| row.info.status.clone())
    })
}

/// Whether the button drawn under `id` is available: a disabled button tracks no focus handle.
fn available(cx: &mut gpui::VisualTestContext, id: &str) -> bool {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .is_some_and(|node| node.flags.focusable)
}

/// Whether the control drawn under `id` takes a click: an unavailable menu item is still focusable (the menu's keys
/// need somewhere to go) but has no click handler.
fn takes_clicks(cx: &mut gpui::VisualTestContext, id: &str) -> bool {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .is_some_and(|node| node.flags.click)
}

/// The row menu's lifecycle commands, which an open lifecycle question refuses (`session_list::menu::refused`).
const ROW_ACTIONS: [&str; 6] = [
    "session-menu-rename",
    "session-menu-clone",
    "session-menu-replace-with",
    "session-menu-replace",
    "session-menu-stop",
    "session-menu-delete",
];

/// Push `fixture` as the open session `a`'s new state, under the listing `a`, `b`.
fn push_a(main: &mut MainWindow, fixture: Fixture) {
    push(&mut main.helm, &["a", "b"], &[(fixture, 0)], main.cx);
}

// ---- The terminal or the card --------------------------------------------------------------------------------------

/// SPEC.md, "Durability and resume": an interrupted session's "terminal contents are gone", and opening it "shows a
/// centered neutral card in the empty terminal area"; "Session view": the pane then shows "the session's metadata and
/// says why there is no terminal, rather than an empty pane". Attaching would only meet the missing terminal, and a tab
/// strip would offer tabs of a terminal that is gone. Specified: with an interrupted a open at startup, the pane opens
/// no stream and mounts no terminal set, draws the card (named by its words, which say what ended the agent) between
/// the top row and the details bar, which still names the session and reads Interrupted; and the card holds focus.
#[gpui::test]
fn an_interrupted_session_opens_on_its_card_with_no_terminal(cx: &mut TestAppContext) {
    let mut main = open_main(
        cx,
        snapshot(&[(interrupted("a"), 0), (session("b"), 0)], false),
    );
    lifecycle(&mut main.helm);
    assert_eq!(streams(&main), [], "no stream");
    assert!(!drawn(main.cx, "terminal-grid"), "no terminal");
    assert!(!drawn(main.cx, STRIP_ID), "no tab strip");
    assert!(
        label_of(main.cx, CARD).is_some_and(|label| label
            .starts_with("A host reboot or a Farhelm update ended this session's agent")),
        "the card names what ended the agent"
    );
    let card = test_support::bounds(main.cx, CARD);
    assert!(card.top() >= test_support::bounds(main.cx, "session-header").bottom());
    assert!(card.bottom() <= test_support::bounds(main.cx, super::details::BAR_ID).top());
    assert!(
        painted(&frame(main.cx))
            .iter()
            .any(|text| text == "Interrupted"),
        "the details bar's badge"
    );
    assert_eq!(focused_id(main.cx).as_deref(), Some(CARD));
}

/// The recovery card is read on focus. A failed hook reason must leave its Replace instruction readable beside
/// right-to-left host text, and assistive technology must name the same escaped error without drawing controls.
#[gpui::test]
fn the_recovery_card_isolates_and_names_a_failed_hook_reason(cx: &mut TestAppContext) {
    use farhelm_proto::ui_channel::Collection;

    let mut state = snapshot(&[(interrupted("a"), 0)], false);
    let row = &mut state
        .iter_mut()
        .find(|collection| collection.collection == Collection::Sessions)
        .expect("the premise: one session collection")
        .entries[0]
        .value;
    row["agent_kind"] = json!("antigravity");
    row["restart_offer"] = json!("not_captured");
    row["persistent_hook_setup"] = json!({"status": "failed", "reason": "تعذّر\u{202e}"});
    let main = open_main(cx, state);
    assert_eq!(focused_id(main.cx).as_deref(), Some(CARD));
    assert!(
        !drawn(main.cx, "session-card-restart"),
        "there is no captured conversation"
    );
    let name = label_of(main.cx, CARD).expect("the card is named by its explanation");
    assert!(
        name.contains(
            "Farhelm could not set up its Antigravity conversation hooks (تعذّر<U+202E>). Replace starts the session over."
        )
    );
    assert!(!name.contains(['\u{2066}', '\u{2069}', '\u{202e}']));
    let nodes = frame(main.cx);
    assert!(
        crate::session_list::host_rail::tests::texts_within(&nodes, "session-recovery-reason")
            .iter()
            .any(|line| line.text.contains("\u{2066}تعذّر<U+202E>\u{2069})"))
    );
    assert!(
        nodes.iter().all(|node| {
            node.element_id.as_deref() != Some("session-recovery-reason") || node.role.is_none()
        }),
        "the parent card already provides the accessible name"
    );
}

/// SPEC.md, "Durability and resume": "Nothing respawns unattended", so looking at an interrupted session is never a
/// request to bring it back, and leaving it is no request to clean it up. The old `terminal-multihost.spec.ts` case
/// "reboot-already-interrupted" (reference commit `24bc0580`) pinned that opening such a session mounts no terminal and
/// leaving it changes nothing; the card's other tests drain what opening sends without looking at it. Specified: with
/// an interrupted a open at startup, the window has sent no Restart, Replace, Stop or Delete; opening b and then a
/// again each send exactly what any open of a seen session sends, its `last_selected` preference and the session query
/// naming it, and nothing else; b's one stream opens and none for a, and a is on its card again.
#[gpui::test]
fn opening_and_leaving_an_interrupted_session_asks_the_helm_for_nothing(cx: &mut TestAppContext) {
    /// Every command the window sent since the last call, as (method, path, body), each answered with success so
    /// nothing waits behind it.
    fn everything(fake: &mut FakeHelm) -> Vec<(Method, String, Value)> {
        let mut found = Vec::new();
        for command in fake.commands() {
            fake.reply(command.id, 200, None);
            found.push((
                command.method,
                command.path,
                command.body.unwrap_or_default(),
            ));
        }
        found
    }

    let mut main = open_main(
        cx,
        snapshot(&[(interrupted("a"), 0), (session("b"), 0)], false),
    );
    assert!(drawn(main.cx, CARD), "the premise: a opens on its card");
    // What the startup sends besides is the selection rules' business; a change to a session is not among it.
    let changes: Vec<String> = everything(&mut main.helm)
        .into_iter()
        .filter(|(method, path, _)| {
            *method == Method::Delete
                || (*method == Method::Post
                    && ["/stop", "/restart", "/replace"]
                        .iter()
                        .any(|verb| path.ends_with(verb)))
        })
        .map(|(_, path, _)| path)
        .collect();
    assert_eq!(changes, Vec::<String>::new(), "opening a at startup");

    for (row, opened) in [("session-row-b", "b"), ("session-row-a", "a")] {
        test_support::click(main.cx, row);
        assert_eq!(selected(main.cx).as_deref(), Some(opened), "the premise");
        let sent: Vec<(String, Value)> = everything(&mut main.helm)
            .into_iter()
            .map(|(_, path, body)| (path, body))
            .collect();
        assert_eq!(
            sent,
            [
                (
                    "/api/preferences".to_owned(),
                    json!({ "last_selected": opened })
                ),
                (
                    farhelm_proto::ui_channel::SESSION_QUERY_PATH.to_owned(),
                    json!({ "selected": opened })
                ),
            ],
            "opening {opened} sends only the open itself"
        );
    }
    assert!(drawn(main.cx, CARD), "a is on its card again");
    assert_eq!(streams(&main).len(), 1, "b's stream alone; none for a");
}

/// The old app dropped an open session's terminal the moment the listing said it was interrupted, and mounted a fresh
/// one once it was not (`header.spec.ts`, "header pressure follows tab strips added and replaced after interruption";
/// `terminal-multihost.spec.ts`, "reboot-while-open" and "reboot-restart"). A terminal kept across the interruption
/// would be attached to one the reboot took; one kept across the recovery would miss the new run's terminal.
/// Specified: with a running a open and live, a push that says a is interrupted closes its stream, draws the card and
/// moves focus from the gone terminal to the card; a push that says a is idle again opens a second stream, attaching,
/// takes the card away, and the new terminal is home and takes focus.
#[gpui::test]
fn interruption_drops_the_terminal_and_recovery_mounts_a_new_one(cx: &mut TestAppContext) {
    let mut main = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    main.terminal_live();
    assert_eq!(streams(&main), [FakePhase::Attached], "the premise");

    push_a(&mut main, interrupted("a"));
    assert_eq!(streams(&main), [FakePhase::Closed]);
    assert!(drawn(main.cx, CARD));
    assert_eq!(focused_id(main.cx).as_deref(), Some(CARD));

    push_a(&mut main, session("a").status(json!({"state": "idle"})));
    assert_eq!(streams(&main), [FakePhase::Closed, FakePhase::Attaching]);
    assert!(!drawn(main.cx, CARD));
    assert!(test_support::terminal_focused(main.cx));
}

/// Stale beats interrupted: the helm cannot ask a host it cannot reach anything, so a session there is the
/// unreachable-host card's whatever its last status said (the old `terminal_absence`), and the terminal it has stays
/// (SPEC.md: "A terminal already open keeps its last screen"). Specified: an open, attached running session whose push
/// says it is interrupted on a stale host keeps its one stream attached and draws no interrupted card.
#[gpui::test]
fn a_session_gone_unreachable_while_open_takes_no_interrupted_card(cx: &mut TestAppContext) {
    let mut main = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    main.terminal_live();
    push_a(&mut main, interrupted("a").stale());
    assert!(!drawn(main.cx, CARD), "gone stale while open");
    assert_eq!(streams(&main), [FakePhase::Attached]);
}

/// The same rule for a session opened on a host the helm cannot reach: its last status said interrupted, but nothing
/// can restart it there, so the interrupted card's offer would be one the helm refuses, and an attach would only meet a
/// host that does not answer. Before the unavailable card, such a session mounted terminals, because only the
/// interrupted card kept them away. Specified: a session opened interrupted on a stale host draws no interrupted card,
/// opens no stream, mounts no tab strip, and draws the unavailable card.
#[gpui::test]
fn a_session_opened_unreachable_takes_no_interrupted_card(cx: &mut TestAppContext) {
    let main = open_main(
        cx,
        snapshot(&[(interrupted("a").stale(), 0), (session("b"), 0)], false),
    );
    assert!(!drawn(main.cx, CARD));
    assert_eq!(streams(&main), [], "no stream");
    assert!(!drawn(main.cx, STRIP_ID), "no tab strip");
    assert!(drawn(main.cx, super::unavailable::CARD_ID));
}

/// A missing agent terminal prevents the pane from dialing or exposing shell tabs. A fresh listing can still
/// publish a terminal with the same Exited status, which gives the pane a screen it can attach to.
#[gpui::test]
fn missing_terminal_opens_on_recovery_until_the_host_reports_one(cx: &mut TestAppContext) {
    let mut main = open_main(
        cx,
        snapshot(&[(missing_terminal("a"), 0), (session("b"), 0)], false),
    );
    assert!(drawn(main.cx, CARD));
    assert!(
        label_of(main.cx, CARD)
            .unwrap()
            .starts_with("This session's terminal is gone")
    );
    assert_eq!(focused_id(main.cx).as_deref(), Some(CARD));
    assert!(drawn(main.cx, "session-card-restart"));
    assert!(drawn(main.cx, "session-card-replace"));
    assert!(
        !drawn(main.cx, STRIP_ID),
        "the still-listed shell tab is hidden"
    );
    assert!(
        streams(&main).is_empty(),
        "there was no attach for either terminal"
    );

    push_a(
        &mut main,
        session("a").status(json!({"state": "exited", "exit_code": 0})),
    );
    assert_eq!(streams(&main), [FakePhase::Attaching]);
    assert!(!drawn(main.cx, CARD));

    push_a(&mut main, missing_terminal("a"));
    assert_eq!(streams(&main), [FakePhase::Closed]);
    assert!(drawn(main.cx, CARD));
    push_a(&mut main, interrupted("a").terminal_missing().tabs(1));
    assert!(
        label_of(main.cx, CARD)
            .unwrap()
            .starts_with("A host reboot or a Farhelm update ended this session's agent")
    );
    assert_eq!(streams(&main), [FakePhase::Closed]);
}

/// Availability from the host still determines whether its recovery controls can be used.
#[gpui::test]
fn a_missing_terminal_on_an_unreachable_host_uses_the_unavailable_card(cx: &mut TestAppContext) {
    let main = open_main(
        cx,
        snapshot(
            &[(missing_terminal("a").stale(), 0), (session("b"), 0)],
            false,
        ),
    );
    assert!(drawn(main.cx, super::unavailable::CARD_ID));
    assert!(!drawn(main.cx, CARD));
    assert!(streams(&main).is_empty());
}

// ---- Restart -------------------------------------------------------------------------------------------------------

/// The helm can keep the last Running status after a relaunch leaves no terminal, but with no terminal no agent runs,
/// so a question saying "Still running — restarting stops the agent" would be false, and the supervisor asks no consent
/// of a session with no terminal. Specified: on a missing-terminal card whose row still reads Running, the card's
/// Restart draws no question and sends exactly one plain Restart, without consent to stop an agent.
#[gpui::test]
fn the_cards_restart_asks_nothing_when_a_missing_terminal_still_reads_running(
    cx: &mut TestAppContext,
) {
    let mut main = open_main(cx, snapshot(&[(session("a").terminal_missing(), 0)], false));
    assert!(drawn(main.cx, CARD));
    assert_eq!(listed_status(main.cx, "a"), Some(SessionStatus::Running));
    lifecycle(&mut main.helm);

    test_support::click(main.cx, "session-card-restart");
    assert!(!drawn(main.cx, "prompt-confirm"));
    assert_eq!(sent(&mut main.helm), [restart("a")]);
}

/// Restart can succeed just before a supervisor shutdown fences off the replacement terminal. The reply's
/// explicit absence keeps the recovery surface visible until there is a terminal to attach to.
#[gpui::test]
fn a_restart_reply_still_missing_its_terminal_keeps_the_card(cx: &mut TestAppContext) {
    let mut main = open_main(cx, snapshot(&[(missing_terminal("a"), 0)], false));
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-restart");
    let [(id, _, _)] = lifecycle(&mut main.helm)[..] else {
        panic!("one Restart was sent");
    };
    main.helm
        .reply(id, 200, Some(json!({"terminal_missing": true})));
    test_support::settle(main.cx);
    assert!(drawn(main.cx, CARD));
    assert!(streams(&main).is_empty());
}

/// A second Restart can run from the header before the listing catches up to the first. If its reply reports
/// absence, the earlier success no longer justifies mounting a terminal.
#[gpui::test]
fn a_newer_restart_reply_reporting_absence_restores_the_card(cx: &mut TestAppContext) {
    let mut main = open_main(cx, snapshot(&[(missing_terminal("a").tabs(0), 0)], false));
    assert!(drawn(main.cx, CARD));
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-restart");
    let [(first, _, _)] = lifecycle(&mut main.helm)[..] else {
        panic!("one Restart was sent from the card");
    };
    main.helm.reply(first, 200, Some(json!({})));
    test_support::settle(main.cx);
    assert!(!drawn(main.cx, CARD));
    assert_eq!(streams(&main), [FakePhase::Attaching]);

    test_support::click(main.cx, "session-header-restart");
    let [(second, _, _)] = lifecycle(&mut main.helm)[..] else {
        panic!("one Restart was sent from the header");
    };
    main.helm
        .reply(second, 200, Some(json!({"terminal_missing": true})));
    test_support::settle(main.cx);
    assert!(drawn(main.cx, CARD));
    assert_eq!(streams(&main), [FakePhase::Closed]);
}

/// Keep the success mark through a repeated listing of the old absence, even if the row says Exited. The first
/// listing with a terminal retires it, so a subsequent loss shows the recovery card again.
#[gpui::test]
fn a_restart_mark_lasts_until_terminal_presence_changes(cx: &mut TestAppContext) {
    let no_terminal = || missing_terminal("a").tabs(0);
    let mut main = open_main(
        cx,
        snapshot(&[(no_terminal(), 0), (session("b"), 0)], false),
    );
    assert!(drawn(main.cx, CARD));
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-restart");
    let [(id, _, _)] = lifecycle(&mut main.helm)[..] else {
        panic!("one Restart was sent");
    };
    main.helm.reply(id, 200, Some(json!({})));
    test_support::settle(main.cx);
    assert!(!drawn(main.cx, CARD));
    assert_eq!(streams(&main), [FakePhase::Attaching]);

    push_a(&mut main, no_terminal().activity(200));
    assert!(!drawn(main.cx, CARD));
    assert_eq!(streams(&main), [FakePhase::Attaching]);
    push_a(
        &mut main,
        session("a").status(json!({"state": "exited", "exit_code": 0})),
    );
    assert_eq!(streams(&main), [FakePhase::Attaching]);
    push_a(&mut main, no_terminal());
    assert!(drawn(main.cx, CARD));
    assert_eq!(streams(&main), [FakePhase::Closed]);
}

/// SPEC.md: "Nothing respawns unattended", and the card's Restart is the user's intentional restart; nothing runs, so
/// it asks nothing (`terminal-restart.spec.ts`, "restart from the interrupted surface sends the resume request exactly
/// once"). Its wait and its refusal move into the card ("the sidebar, session header and interrupted-session card
/// share a spinner, the host and elapsed seconds"), so the header must not draw a second copy. Specified: a click on
/// the card's Restart sends exactly one plain Restart without consent and opens no question; at two seconds the card
/// says "Restarting on buildbox…" and the header draws no wait line; once the helm refuses, the card draws "Restart:
/// the host refused this operation" inside itself.
#[gpui::test]
fn the_cards_restart_is_one_click_and_its_wait_and_refusal_show_in_the_card(
    cx: &mut TestAppContext,
) {
    let mut main = open_main(cx, snapshot(&[(interrupted("a"), 0)], false));
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-restart");
    let [(id, path, body)] = &lifecycle(&mut main.helm)[..] else {
        panic!("one Restart was sent");
    };
    assert_eq!((path.clone(), body.clone()), restart("a"));
    assert!(!any_layer(main.cx), "no question");

    test_support::advance(main.cx, PROGRESS_SHOWS_AFTER);
    assert_eq!(
        label_of(main.cx, "session-card-restart-progress-label-a").as_deref(),
        Some("Restarting on buildbox…")
    );
    assert_eq!(
        label_of(main.cx, "session-header-restart-progress-label-a"),
        None,
        "the header leaves its copy out"
    );

    main.helm
        .reply(*id, 409, Some(json!("the host refused this operation")));
    test_support::settle(main.cx);
    assert_eq!(
        label_of(main.cx, "session-header-outcome-a").as_deref(),
        Some("Restart: the host refused this operation")
    );
    assert!(drawn_inside(main.cx, "session-header-outcome-a", CARD));
}

/// The helm answers a Restart before its list stops saying Interrupted: the supervisor's reply carries an Unknown
/// status, which the helm's cache never lets overwrite a definite one, and the real status comes with the host's next
/// listing. The old card bridged that gap with its `relaunched` mark (`terminal-restart.spec.ts`: "card gone and
/// terminal back on the reply, ahead of the listing"); without one the card comes back with Restart enabled, and a
/// second click restarts a session that is already running. (That spec's "still one request after 500 ms" guarded
/// against a browser click delivered twice, which nothing here can do; the card's Restart being gone is the check.)
/// Specified, in the helm's order: once the card's Restart is answered with success while the store still lists a
/// interrupted (asserted, since that is the gap), the card and its Restart are gone and one stream is attaching for a's
/// new terminal; the push that lists a live keeps that one stream; and a later push that lists a interrupted again
/// shows the card again, since the live status ended the mark.
#[gpui::test]
fn a_restart_leaves_the_card_on_its_reply_until_the_list_catches_up(cx: &mut TestAppContext) {
    let mut main = open_main(
        cx,
        snapshot(&[(interrupted("a"), 0), (session("b"), 0)], false),
    );
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-restart");
    let [(id, _, _)] = lifecycle(&mut main.helm)[..] else {
        panic!("one Restart was sent");
    };
    main.helm.reply(id, 200, Some(json!({})));
    test_support::settle(main.cx);
    assert_eq!(
        listed_status(main.cx, "a"),
        Some(SessionStatus::Interrupted),
        "the premise: the store still lists a interrupted"
    );
    assert!(!drawn(main.cx, CARD), "gone on the reply");
    assert!(!drawn(main.cx, "session-card-restart"));
    assert_eq!(streams(&main), [FakePhase::Attaching]);

    push_a(&mut main, session("a"));
    assert_eq!(streams(&main), [FakePhase::Attaching]);
    push_a(&mut main, interrupted("a"));
    assert!(drawn(main.cx, CARD), "the mark ended with the live status");
}

/// The mark's other end, the old one's second-reboot reason: a host that drops away and comes back rebooted again
/// lists the session Interrupted with no live status in between, and a mark kept across that would hide the card and
/// attach to a terminal the second reboot took. Specified: after the card's Restart succeeded, a push that lists a
/// interrupted on an unreachable host and then one that lists it interrupted on a reachable host show the card again.
#[gpui::test]
fn a_restarted_session_whose_host_drops_away_shows_its_card_again(cx: &mut TestAppContext) {
    let mut main = open_main(
        cx,
        snapshot(&[(interrupted("a"), 0), (session("b"), 0)], false),
    );
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-restart");
    let [(id, _, _)] = lifecycle(&mut main.helm)[..] else {
        panic!("one Restart was sent");
    };
    main.helm.reply(id, 200, Some(json!({})));
    test_support::settle(main.cx);
    assert!(!drawn(main.cx, CARD), "the premise: marked restarted");
    push_a(&mut main, interrupted("a").stale());
    push_a(&mut main, interrupted("a"));
    assert!(drawn(main.cx, CARD));
}

/// A card whose host drops away must not turn into a terminal: the reboot took the terminal there too, so an attach
/// would only meet its absence, and the next answer from the host may still say interrupted (the old view kept a
/// surface across an outage only when it already had one, which an interrupted view never has). Specified: from an
/// interrupted a on its card, a push that lists it on an unreachable host opens no stream and draws no card; one that
/// lists it reachable and still interrupted draws the card and opens no stream; one that lists it idle opens one.
#[gpui::test]
fn an_interrupted_session_whose_host_drops_away_mounts_no_terminal(cx: &mut TestAppContext) {
    let mut main = open_main(
        cx,
        snapshot(&[(interrupted("a"), 0), (session("b"), 0)], false),
    );
    assert!(drawn(main.cx, CARD), "the premise");
    push_a(&mut main, interrupted("a").stale());
    assert_eq!(streams(&main), [], "unreachable");
    assert!(!drawn(main.cx, CARD));
    push_a(&mut main, interrupted("a"));
    assert_eq!(streams(&main), [], "back, still interrupted");
    assert!(drawn(main.cx, CARD));
    push_a(&mut main, session("a").status(json!({"state": "idle"})));
    assert_eq!(streams(&main), [FakePhase::Attaching]);
}

/// The card's question while its host drops away: the interrupted card goes (the unavailable card takes its place,
/// which has no question to draw), so the question goes with it, or it would refuse every lifecycle command of a's with
/// nothing on screen to answer until the host came back. The old card cleared its Replace prompt the moment it was
/// hidden (`use_clear_when_hidden`), and a stale row hid it. Specified: with the card's Replace question open on a, a
/// push that lists a on an unreachable host takes the card and the question away and shows the unavailable card, and
/// a's menu then offers Rename, Clone, Replace with…, Replace, Stop and Delete; once a is listed reachable and
/// interrupted again, the card shows with no question open.
#[gpui::test]
fn the_cards_question_goes_when_its_host_drops_away(cx: &mut TestAppContext) {
    let mut main = open_main(
        cx,
        snapshot(&[(interrupted("a"), 0), (session("b"), 0)], false),
    );
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-replace");
    assert!(
        drawn(main.cx, "prompt-confirm"),
        "the premise: the card asks"
    );

    push_a(&mut main, interrupted("a").stale());
    lifecycle(&mut main.helm);
    assert!(
        !drawn(main.cx, CARD),
        "the premise: no card while unreachable"
    );
    assert!(
        drawn(main.cx, super::unavailable::CARD_ID),
        "the unavailable card in its place"
    );
    assert!(!drawn(main.cx, "prompt-confirm"), "no question is drawn");
    test_support::click(main.cx, "session-menu-a");
    for item in ROW_ACTIONS {
        assert!(takes_clicks(main.cx, item), "{item} is available");
    }
    test_support::keys(main.cx, "escape");

    push_a(&mut main, interrupted("a"));
    lifecycle(&mut main.helm);
    assert!(drawn(main.cx, CARD), "the premise: the card is back");
    assert!(!drawn(main.cx, "prompt-confirm"), "with no question open");
}

// ---- Replace -------------------------------------------------------------------------------------------------------

/// SPEC.md: Replace "keeps its inline destructive confirmation", and the old card asked it inside the card, without
/// the quoted title the row's question carries (`terminal-restart.spec.ts`, "Replace confirms inline, can cancel,
/// selects the fresh session, and surfaces refusal"). Specified: a click on the card's Replace draws the question inside
/// the card with Replace's consequence for an interrupted session and no title line, focused on Confirm replace; its
/// Cancel sends nothing and takes it away; asked again, Enter sends exactly one Replace with the nothing-alive guard
/// the interrupted row shows.
#[gpui::test]
fn the_cards_replace_asks_inline_and_sends_the_guard_it_froze(cx: &mut TestAppContext) {
    let mut main = open_main(cx, snapshot(&[(interrupted("a"), 0)], false));
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-replace");
    assert!(drawn_inside(main.cx, "prompt-confirm", CARD));
    assert!(
        painted(&frame(main.cx)).iter().any(|text| text
            == "Interrupted by a host reboot or a Farhelm update, which ended the agent, but replacing still discards \
                the conversation; a fresh session with the same settings takes its place:")
    );
    assert_eq!(
        label_of(main.cx, "session-replace-title-a"),
        None,
        "no title line"
    );
    assert_eq!(focused_id(main.cx).as_deref(), Some("prompt-confirm"));

    test_support::click(main.cx, "prompt-cancel");
    assert_eq!(sent(&mut main.helm), []);
    assert!(!drawn(main.cx, "prompt-confirm"));

    test_support::click(main.cx, "session-card-replace");
    assert_eq!(focused_id(main.cx).as_deref(), Some("prompt-confirm"));
    test_support::keys(main.cx, "enter");
    assert_eq!(sent(&mut main.helm), [replace("a", true, false)]);
}

/// SPEC.md, "Lifecycle operations": Skip session confirmations skips "interrupted-card Replace" too
/// (`session-confirmations.spec.ts`: the card's Replace "sends at once" with the drawn row's guard). Specified: with
/// the preference on, a click on the card's Replace sends one Replace with the nothing-alive guard and draws no
/// question.
#[gpui::test]
fn skipped_confirmations_replace_from_the_card_at_once(cx: &mut TestAppContext) {
    let mut main = open_main(cx, snapshot(&[(interrupted("a"), 0)], true));
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-replace");
    assert_eq!(sent(&mut main.helm), [replace("a", true, false)]);
    assert!(!drawn(main.cx, "prompt-confirm"));
}

/// SPEC.md, "Creation": the YOLO question shows "with the control or surface that started the launch", and the
/// migration's orchestrator decided on 2026-11-14 that the card's Replace asks it as the row and the header do. Shown
/// below the top row instead, it would draw over nothing the user used. Specified: a YOLO refusal of the card's
/// confirmed Replace draws one question, inside the card; its one-off answer sends the Replace again with consent and
/// the guard the card's question froze.
#[gpui::test]
fn a_yolo_refused_replace_from_the_card_asks_in_the_card(cx: &mut TestAppContext) {
    let mut main = open_main(cx, snapshot(&[(interrupted("a"), 0)], false));
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-replace");
    test_support::click(main.cx, "prompt-confirm");
    let [(id, _, _)] = lifecycle(&mut main.helm)[..] else {
        panic!("one Replace was sent");
    };
    refuse_for_yolo(&mut main.helm, main.cx, id);
    let questions = frame(main.cx)
        .iter()
        .filter(|node| node.element_id.as_deref() == Some("prompt-confirm"))
        .count();
    assert_eq!(questions, 1, "one question");
    assert!(drawn_inside(main.cx, "prompt-confirm", CARD));
    test_support::click(main.cx, "prompt-confirm");
    assert_eq!(sent(&mut main.helm), [consented("a", true, false)]);
}

/// SPEC.md, "Opening and closing": closing something returns focus to where it was before it opened, "on every platform,
/// including those where clicking the button that opened it did not move focus to the button". On the Mac a click never
/// focuses the answer it presses, so a pointer answer and a keyboard answer reach the question's close with focus in
/// different places, and the Linux suite, where a click focuses the answer first, cannot tell them apart. This guards
/// the Mac click rule on a question drawn in the page itself (the card's inline YOLO question) rather than in a dialog,
/// which the prompt's own Mac-rule test covers. Specified, under the macOS click rule: the card's
/// Replace, confirmed and refused for YOLO, asks with focus on Cancel; a click on Cancel closes the question, sends
/// nothing and leaves focus where it was when the question arrived; asked again, Enter on Cancel does the same.
#[gpui::test]
fn a_clicked_yolo_answer_in_the_card_returns_focus_as_enter_does_on_the_mac(
    cx: &mut TestAppContext,
) {
    let mut main = open_main(cx, snapshot(&[(interrupted("a"), 0)], false));
    main.cx
        .update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    lifecycle(&mut main.helm);
    for clicked in [true, false] {
        test_support::click(main.cx, "session-card-replace");
        test_support::click(main.cx, "prompt-confirm");
        let [(id, _, _)] = lifecycle(&mut main.helm)[..] else {
            panic!("one Replace was sent");
        };
        let arrived = focused_id(main.cx);
        assert!(arrived.is_some(), "the premise: a control holds focus");
        refuse_for_yolo(&mut main.helm, main.cx, id);
        assert_eq!(
            focused_id(main.cx).as_deref(),
            Some("prompt-cancel"),
            "the premise: the question took focus"
        );
        if clicked {
            test_support::click(main.cx, "prompt-cancel");
        } else {
            test_support::keys(main.cx, "enter");
        }
        assert!(
            !drawn(main.cx, "prompt-cancel"),
            "clicked {clicked}: closed"
        );
        assert_eq!(sent(&mut main.helm), [], "clicked {clicked}: nothing sent");
        assert_eq!(focused_id(main.cx), arrived, "clicked {clicked}");
    }
}

/// The follower notifies on the host's reachability as well as on the card, for the one path where the card does not
/// change: a card whose host drops away (no card, nothing mounted) and whose session is listed live once the host is
/// back, after a restart elsewhere or pushes that came together. Without it the pane would stay blank until some
/// other change. Specified: from an interrupted a on its card, a push that lists it on an unreachable host opens no
/// stream, and one that lists it idle on a reachable host opens one.
#[gpui::test]
fn a_session_back_from_unreachable_already_live_mounts_its_terminal(cx: &mut TestAppContext) {
    let mut main = open_main(
        cx,
        snapshot(&[(interrupted("a"), 0), (session("b"), 0)], false),
    );
    push_a(&mut main, interrupted("a").stale());
    assert_eq!(
        streams(&main),
        [],
        "the premise: unreachable, nothing mounted"
    );
    push_a(&mut main, session("a").status(json!({"state": "idle"})));
    assert_eq!(streams(&main), [FakePhase::Attaching]);
}

/// The helm removes a Replace's source before it replies, and the list keeps the selection on the source until the
/// reply (`session_list::selection`), so for a moment the open session has no row at all. That moment must change
/// nothing: terminals mounted then would attach to the session the helm just removed, and the old view kept its card
/// up with the Replace's progress until the reply (`terminal-restart.spec.ts`, "Replace confirms inline ... selects the
/// fresh session"). Specified: after the card's confirmed Replace of a, a push that removes a and lists its replacement
/// n mounts no terminals (no tab strip) and opens no stream, and keeps a selected and its card drawn; the reply then
/// selects n, the card goes, and one stream opens, for n (the first stream of the test, opened once n is selected).
#[gpui::test]
fn a_replace_from_the_card_holds_until_its_reply_and_opens_the_replacement(
    cx: &mut TestAppContext,
) {
    let mut main = open_main(
        cx,
        snapshot(&[(interrupted("a"), 0), (session("b"), 0)], false),
    );
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-replace");
    test_support::click(main.cx, "prompt-confirm");
    let [(id, _, _)] = lifecycle(&mut main.helm)[..] else {
        panic!("one Replace was sent");
    };
    main.helm.push(change(
        listing_meta(&["n", "b"], 2, false),
        &[session("n")],
        &["a"],
    ));
    test_support::settle(main.cx);
    assert_eq!(
        selected(main.cx).as_deref(),
        Some("a"),
        "the premise: the source stays open"
    );
    assert_eq!(streams(&main), [], "no stream for the removed source");
    // A mounted set draws its tab strip in the top row even while the held card keeps its terminal from being drawn,
    // and so from opening its stream; the strip is what shows terminals were mounted for the removed source.
    assert!(
        !drawn(main.cx, STRIP_ID),
        "no terminals mounted for the removed source"
    );
    assert!(drawn(main.cx, CARD), "the card holds");

    main.helm.reply(id, 200, Some(session("n").entry().value));
    test_support::settle(main.cx);
    assert_eq!(selected(main.cx).as_deref(), Some("n"));
    assert!(!drawn(main.cx, CARD));
    // The only stream, opened after the reply moved the selection: n's.
    assert_eq!(streams(&main), [FakePhase::Attaching]);
}

/// A restarted mark matters only while its session's card would show, and a session that left the listing (a filter,
/// the list's cap, another session opened) has no card to hide; kept, its mark would outlive the gap it bridges and hide
/// the card when a later reboot lists it interrupted again. Specified: b, open and interrupted, is restarted from its
/// card with success while the store still lists it interrupted; a is opened, a push takes b out of the listing, a later
/// one lists b interrupted again, and opening b shows its card.
#[gpui::test]
fn a_restarted_session_that_leaves_the_listing_shows_its_card_again(cx: &mut TestAppContext) {
    let mut main = open_main(
        cx,
        snapshot(&[(interrupted("b"), 0), (session("a"), 0)], false),
    );
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-restart");
    let [(id, _, _)] = lifecycle(&mut main.helm)[..] else {
        panic!("one Restart was sent");
    };
    main.helm.reply(id, 200, Some(json!({})));
    test_support::settle(main.cx);
    assert!(!drawn(main.cx, CARD), "the premise: b is marked restarted");

    test_support::click(main.cx, "session-row-a");
    test_support::settle(main.cx);
    lifecycle(&mut main.helm);
    main.helm
        .push(change(listing_meta(&["a"], 1, false), &[], &["b"]));
    test_support::settle(main.cx);
    push(
        &mut main.helm,
        &["b", "a"],
        &[(interrupted("b"), 0)],
        main.cx,
    );
    test_support::click(main.cx, "session-row-b");
    test_support::settle(main.cx);
    lifecycle(&mut main.helm);
    assert_eq!(
        selected(main.cx).as_deref(),
        Some("b"),
        "the premise: b is open"
    );
    assert!(drawn(main.cx, CARD));
}

/// The restarted mark is this window's own knowledge, and a reset throws away everything the window held from the helm
/// but the selection (SPEC.md, "Losing the connection to the helm"); a mark kept across it would hide the card of a
/// session the new snapshot lists interrupted. Specified: after the card's Restart succeeds (the card gone, one stream
/// attaching), the channel breaks and reconnects with a snapshot that lists a interrupted; the card shows again and no
/// second stream opens.
#[gpui::test]
fn a_reset_forgets_that_the_session_was_restarted(cx: &mut TestAppContext) {
    let mut main = open_main(
        cx,
        snapshot(&[(interrupted("a"), 0), (session("b"), 0)], false),
    );
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-restart");
    let [(id, _, _)] = lifecycle(&mut main.helm)[..] else {
        panic!("one Restart was sent");
    };
    main.helm.reply(id, 200, Some(json!({})));
    test_support::settle(main.cx);
    assert!(!drawn(main.cx, CARD), "the premise: marked restarted");
    assert_eq!(streams(&main).len(), 1, "the premise: one stream");

    main.helm.break_channel();
    test_support::settle(main.cx);
    main.helm
        .connect(snapshot(&[(interrupted("a"), 0), (session("b"), 0)], false));
    test_support::settle(main.cx);
    lifecycle(&mut main.helm);
    assert!(drawn(main.cx, CARD));
    assert_eq!(streams(&main).len(), 1, "no second stream");
}

/// SPEC.md: "After about two seconds waiting for the reply, the sidebar, session header and interrupted-session card
/// share a spinner, the host and elapsed seconds", and "A refusal keeps its ordinary explanation"
/// (`restart-replace-progress.spec.ts`, "Replace from the interrupted card shares progress and clears it on a
/// refusal"; `terminal-restart.spec.ts`: a refusal shows "and Replace is still offered"). Specified: the card's
/// confirmed Replace, unanswered, draws "Replacing on buildbox…" in the card at two seconds and no header copy; once
/// the helm refuses, the card draws "Replace: the host refused this operation" inside itself and still offers Replace.
#[gpui::test]
fn the_cards_replace_shows_its_wait_and_refusal_in_the_card(cx: &mut TestAppContext) {
    let mut main = open_main(cx, snapshot(&[(interrupted("a"), 0)], false));
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-replace");
    test_support::click(main.cx, "prompt-confirm");
    let [(id, _, _)] = lifecycle(&mut main.helm)[..] else {
        panic!("one Replace was sent");
    };
    test_support::advance(main.cx, PROGRESS_SHOWS_AFTER);
    assert_eq!(
        label_of(main.cx, "session-card-replace-progress-label-a").as_deref(),
        Some("Replacing on buildbox…")
    );
    assert_eq!(
        label_of(main.cx, "session-header-replace-progress-label-a"),
        None
    );

    main.helm
        .reply(id, 409, Some(json!("the host refused this operation")));
    test_support::settle(main.cx);
    assert_eq!(
        label_of(main.cx, "session-header-outcome-a").as_deref(),
        Some("Replace: the host refused this operation")
    );
    assert!(drawn_inside(main.cx, "session-header-outcome-a", CARD));
    assert!(
        drawn(main.cx, "session-card-replace"),
        "Replace is still offered"
    );
}

/// A legacy launch cannot be copied as it is, so plain Replace would only meet the helm's refusal; the old card offered
/// the launcher's Replace with in its place (`terminal-restart.spec.ts`, "an interrupted legacy session offers Replace
/// with": "no plain Replace"). Specified: an interrupted session whose launch is legacy draws the card's Replace with
/// and no plain Replace.
#[gpui::test]
fn a_legacy_sessions_card_offers_replace_with_in_place_of_replace(cx: &mut TestAppContext) {
    let legacy = json!({"kind": "legacy", "invocation": "claude", "agent_kind": "claude", "resume_template": null});
    let main = open_main(cx, snapshot(&[(interrupted("a").launch(legacy), 0)], false));
    assert!(drawn(main.cx, "session-card-replace-with"));
    assert!(!drawn(main.cx, "session-card-replace"));
}

/// The rest of the same old case, "prefilled with its command": the card's words promise that "Replace with opens the
/// launcher with its command to start it over", and a legacy launch keeps nothing else to start from, so the launcher
/// must open on that command, for this session. Specified: on an interrupted legacy session a whose stored command is
/// `claude`, a click on the card's Replace with opens the launcher in Replace with of a, on the Command tab, its Agent
/// command field showing `claude`.
#[gpui::test]
fn the_cards_replace_with_opens_the_launcher_on_the_sessions_command(cx: &mut TestAppContext) {
    let legacy = json!({"kind": "legacy", "invocation": "claude", "agent_kind": "claude", "resume_template": null});
    let mut main = open_main(cx, snapshot(&[(interrupted("a").launch(legacy), 0)], false));
    lifecycle(&mut main.helm);
    assert!(!launcher_drawn(main.cx), "the premise: no launcher is open");
    test_support::click(main.cx, "session-card-replace-with");
    assert!(launcher_drawn(main.cx), "Replace with opened the launcher");
    let draft = crate::launcher::view_tests::draft(main.cx);
    assert_eq!(draft.mode, Mode::ReplaceWith { source: "a".into() });
    assert_eq!(draft.tab, Tab::Command);
    let command: Vec<String> =
        crate::session_list::host_rail::tests::texts_within(&frame(main.cx), "launcher-command")
            .into_iter()
            .map(|text| text.text.to_string())
            .collect();
    assert_eq!(command, ["claude"], "the Agent command field");
}

/// The card's Replace with is a New handoff like the header's (SPEC.md: an accepted "New handoff" returns the sidebar
/// to Sessions). The GPUI migration's orchestrator counted the header's three on 2026-11-16; the card's took the same
/// path in the old app, which hid Hosts, and the sidebar-and-session-view area applied the rule to it on 2026-11-17.
/// The card sits beside the sidebar, so it can be pressed while Hosts shows, and the replacement must then appear
/// where the user can see it. Specified: on a legacy session's card with host1 listed and Hosts shown, the card's
/// Replace with opens the launcher and Sessions shows in place of the panel.
#[gpui::test]
fn the_cards_replace_with_from_hosts_shows_sessions(cx: &mut TestAppContext) {
    let legacy = json!({"kind": "legacy", "invocation": "claude", "agent_kind": "claude", "resume_template": null});
    let mut collections = snapshot(&[(interrupted("a").launch(legacy), 0)], false);
    collections.push(fixture_hosts(&[0]));
    let main = open_main(cx, collections);
    test_support::click(main.cx, "hosts-panel-toggle");
    assert!(panel_shown(main.cx), "the premise: Hosts shows");
    test_support::click(main.cx, "session-card-replace-with");
    assert!(launcher_drawn(main.cx), "Replace with opened the launcher");
    assert!(!panel_shown(main.cx), "and Sessions shows");
}

/// SPEC.md: "an open lifecycle confirmation or a pending request refuses another lifecycle action". The card's inline
/// question is no layer, so a press elsewhere does not close it as it closes the header's anchored ones; the header must
/// refuse on its own. Specified: with the card's Replace question open, the header's Restart, Replace and Delete are
/// drawn unavailable (a disabled button tracks no focus) where they were available before; presses on them, and the
/// Delete shortcut, ask nothing and send nothing, and the card's question stays open.
#[gpui::test]
fn the_cards_open_question_leaves_the_headers_actions_unavailable(cx: &mut TestAppContext) {
    let mut main = open_main(cx, snapshot(&[(interrupted("a"), 0)], false));
    lifecycle(&mut main.helm);
    let header = [
        "session-header-restart",
        "session-header-replace",
        "session-header-delete",
    ];
    for button in header {
        assert!(
            available(main.cx, button),
            "the premise: {button} is available"
        );
    }
    test_support::click(main.cx, "session-card-replace");
    assert!(
        drawn(main.cx, "prompt-confirm"),
        "the premise: the card asks"
    );
    for button in header {
        assert!(!available(main.cx, button), "{button} is drawn unavailable");
        test_support::click(main.cx, button);
        assert!(!any_layer(main.cx), "{button} asks nothing");
    }
    test_support::keys(main.cx, DELETE_SESSION_CHORD);
    assert!(!any_layer(main.cx), "the Delete shortcut asks nothing");
    assert_eq!(sent(&mut main.helm), []);
    assert!(drawn_inside(main.cx, "prompt-confirm", CARD));
}

/// The same rule for the row menu: the card's question is inline, so the press on a row's ⋯ that opens its menu leaves
/// it open, and a row action taken meanwhile would run beside a question whose answer still sends the guard it froze.
/// Specified: a's row menu offers Rename, Clone, Replace with…, Replace, Stop and Delete; with the card's Replace
/// question open, each is drawn unavailable (no click handler), a click on each leaves the menu open, nothing is sent,
/// and the card's question is still open once Escape closes the menu; once the question is cancelled, a's menu offers
/// all six again.
#[gpui::test]
fn the_cards_open_question_leaves_the_row_menus_actions_unavailable(cx: &mut TestAppContext) {
    let mut main = open_main(cx, snapshot(&[(interrupted("a"), 0)], false));
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-menu-a");
    for item in ROW_ACTIONS {
        assert!(
            takes_clicks(main.cx, item),
            "the premise: {item} is available"
        );
    }
    test_support::keys(main.cx, "escape");

    test_support::click(main.cx, "session-card-replace");
    assert!(
        drawn(main.cx, "prompt-confirm"),
        "the premise: the card asks"
    );
    test_support::click(main.cx, "session-menu-a");
    assert!(
        drawn(main.cx, "session-menu"),
        "the premise: the menu opened"
    );
    for item in ROW_ACTIONS {
        assert!(!takes_clicks(main.cx, item), "{item} is drawn unavailable");
        test_support::click(main.cx, item);
        assert!(
            drawn(main.cx, "session-menu"),
            "{item} does nothing: the menu is still open"
        );
    }
    assert_eq!(sent(&mut main.helm), []);
    test_support::keys(main.cx, "escape");
    assert!(
        !drawn(main.cx, "session-menu"),
        "the premise: Escape closed the menu"
    );
    assert!(
        drawn_inside(main.cx, "prompt-confirm", CARD),
        "the card still asks"
    );

    test_support::click(main.cx, "prompt-cancel");
    assert!(!drawn(main.cx, "prompt-confirm"), "the premise: cancelled");
    test_support::click(main.cx, "session-menu-a");
    for item in ROW_ACTIONS {
        assert!(takes_clicks(main.cx, item), "{item} is available again");
    }
    assert_eq!(sent(&mut main.helm), []);
}

/// The card's question goes with its card, and a question dropped unanswered still reads open: kept on, it would go
/// on refusing the actions of a session the user can no longer see, for the rest of the UI session. The pane drops it
/// when the selection moves and whenever the card does not show; this pins one of the paths where the card goes with
/// no answer and no recovery, its session deleted elsewhere. Specified: with the card's Replace question open on a, a
/// complete listing pushed without a opens b, and a's lifecycle commands are no longer refused.
#[gpui::test]
fn the_cards_question_goes_when_its_session_is_deleted_elsewhere(cx: &mut TestAppContext) {
    let mut main = open_main(
        cx,
        snapshot(&[(interrupted("a"), 0), (session("b"), 0)], false),
    );
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-replace");
    assert!(
        main.cx.update(|_, cx| menu::refused("a", cx)),
        "the premise: the card asks, which refuses a's actions"
    );

    main.helm
        .push(change(listing_meta(&["b"], 1, false), &[], &["a"]));
    test_support::settle(main.cx);
    lifecycle(&mut main.helm);
    assert_eq!(
        selected(main.cx).as_deref(),
        Some("b"),
        "the premise: a is gone and b is open"
    );
    assert!(!main.cx.update(|_, cx| menu::refused("a", cx)));
}

/// The user's own way of leaving the card while it asks: opening another session. a's row stays listed, so a question
/// kept on would show as a's whole menu refused with nothing on screen to answer. The other session shows a card of
/// its own here, so only the drop on a change of selection takes a's question away, not the one for a card that does
/// not show. Specified: with a's card asking, a click on b's row (b interrupted too) opens b on its card with no
/// question; a's menu then offers Rename, Clone, Replace with…, Replace, Stop and Delete; and opening a again shows its
/// card with no question.
#[gpui::test]
fn the_cards_question_goes_when_another_session_is_opened(cx: &mut TestAppContext) {
    let mut main = open_main(
        cx,
        snapshot(&[(interrupted("a"), 0), (interrupted("b"), 0)], false),
    );
    lifecycle(&mut main.helm);
    test_support::click(main.cx, "session-card-replace");
    assert!(
        drawn(main.cx, "prompt-confirm"),
        "the premise: the card asks"
    );

    test_support::click(main.cx, "session-row-b");
    test_support::settle(main.cx);
    lifecycle(&mut main.helm);
    assert_eq!(
        selected(main.cx).as_deref(),
        Some("b"),
        "the premise: b is open"
    );
    assert!(drawn(main.cx, CARD), "the premise: b shows its card");
    assert!(!drawn(main.cx, "prompt-confirm"), "b's card asks nothing");
    test_support::click(main.cx, "session-menu-a");
    for item in ROW_ACTIONS {
        assert!(takes_clicks(main.cx, item), "a's {item} is available");
    }
    test_support::keys(main.cx, "escape");

    test_support::click(main.cx, "session-row-a");
    test_support::settle(main.cx);
    lifecycle(&mut main.helm);
    assert!(drawn(main.cx, CARD), "the premise: a shows its card");
    assert!(!drawn(main.cx, "prompt-confirm"), "a's card asks nothing");
}

/// Restart and Replace start a session, and SPEC.md, "Creation", says "Launches and changes to the host registry still
/// exclude each other"; the migration's orchestrator decided on 2026-11-15 that they wait for a host change as the
/// launcher's Launch does, giving its reason, on every surface that offers them. Specified: with Add host's probe in
/// flight, the card's Restart and Replace each have "Wait for the host change to finish" as their hover help, a click
/// on each opens nothing, and nothing is sent; once the probe is answered, the card's Restart sends a's Restart.
#[gpui::test]
fn the_cards_restart_and_replace_wait_for_a_host_change(cx: &mut TestAppContext) {
    const WAIT: &str = "Wait for the host change to finish";
    let mut main = open_main(cx, snapshot(&[(interrupted("a"), 0)], false));
    lifecycle(&mut main.helm);
    let path = start_host_change(main.cx, HostChange::Probe);
    let [probe] = &main.helm.commands()[..] else {
        panic!("the premise: only the probe went out");
    };
    assert_eq!(probe.path, path, "the premise: the probe is in flight");
    let probe = probe.id;

    for button in ["session-card-restart", "session-card-replace"] {
        assert_eq!(hover(main.cx, button).as_deref(), Some(WAIT), "{button}");
        test_support::click(main.cx, button);
        assert!(
            !any_layer(main.cx) && !drawn(main.cx, "prompt-confirm"),
            "{button} asks nothing"
        );
    }
    let meanwhile = main.helm.commands();
    assert!(
        meanwhile
            .iter()
            .all(|command| command.method != Method::Post
                || !command.path.ends_with("/restart") && !command.path.ends_with("/replace")),
        "nothing was sent during the probe"
    );

    // The fake answers in order: the probe first, then whatever else went out behind it.
    main.helm.reply(probe, 200, Some(json!({})));
    for command in meanwhile {
        main.helm.reply(command.id, 200, None);
    }
    test_support::settle(main.cx);
    test_support::click(main.cx, "session-card-restart");
    assert_eq!(sent(&mut main.helm), [restart("a")]);
}

/// The other cause of the launcher's wait: a launch the user has asked for and the helm has not answered. The launcher
/// holds one launch at a time, so a legacy session's Replace with waits for it, and must say so rather than look ready
/// while the launcher refuses it, as the row's and the header's Replace with do (SPEC.md, "Terminal experience": greyed
/// entries' "tooltips and accessible descriptions explain the specific reason"). Specified: on a legacy session's card
/// with host1 listed, a command launch opened in the launcher and launched sends one create, which stays unanswered;
/// the card's Replace with, still focusable, then says "Wait for the pending launch to finish", and a click on it opens
/// nothing.
#[gpui::test]
fn the_cards_replace_with_says_why_it_waits_while_a_launch_is_pending(cx: &mut TestAppContext) {
    let legacy = json!({"kind": "legacy", "invocation": "claude", "agent_kind": "claude", "resume_template": null});
    let mut collections = snapshot(&[(interrupted("a").launch(legacy), 0)], false);
    collections.push(fixture_hosts(&[0]));
    let mut main = open_main(cx, collections);
    creates_answering(&mut main.helm, main.cx, &json!([]));
    assert_eq!(
        hover(main.cx, "session-card-replace-with").as_deref(),
        Some("Replace with: edit the settings, then swap this session for a new one"),
        "the premise: Replace with is ready"
    );

    // The smallest complete draft: a Command tab launch of `true`, not YOLO, on host1 in `~`.
    let mut draft = LaunchDraft::new(Some(1), "~".into(), None);
    draft.choose_tab(Tab::Command);
    draft.edit_command("true".into());
    draft.choose_command_yolo(false);
    let opened = main
        .cx
        .update(|window, cx| crate::launcher::entry::open_with(draft, None, window, cx));
    assert!(opened, "the premise: the launcher opened");
    test_support::settle(main.cx);
    test_support::click(main.cx, "launcher-launch");
    let creates = creates_answering(&mut main.helm, main.cx, &json!([]));
    assert_eq!(creates.len(), 1, "the premise: one create is pending");
    assert!(!launcher_drawn(main.cx), "the premise: the launcher hid");

    assert!(
        available(main.cx, "session-card-replace-with"),
        "Replace with stays keyboard-reachable"
    );
    assert_eq!(
        hover(main.cx, "session-card-replace-with").as_deref(),
        Some("Wait for the pending launch to finish")
    );
    test_support::click(main.cx, "session-card-replace-with");
    assert!(
        !launcher_drawn(main.cx),
        "a click on Replace with opens nothing"
    );
}

// ---- Focus ---------------------------------------------------------------------------------------------------------

/// SPEC.md, "Where focus rests": with no terminal on screen focus rests "on a stopped session's card", "Enter on that
/// card does nothing, so typing that was already under way cannot restart anything; Tab reaches its Restart and
/// Replace"; and Escape on the card's question is its Cancel (SPEC.md, "Opening and closing"). Specified: Enter on the
/// focused card sends nothing and opens nothing; Tab moves to the card's Restart, then its Replace; Enter there opens
/// the question on Confirm replace, and Escape takes it away, sends nothing and leaves focus on Replace.
#[gpui::test]
fn the_card_is_home_enter_does_nothing_and_tab_reaches_restart_then_replace(
    cx: &mut TestAppContext,
) {
    let mut main = open_main(cx, snapshot(&[(interrupted("a"), 0)], false));
    lifecycle(&mut main.helm);
    assert_eq!(focused_id(main.cx).as_deref(), Some(CARD), "the premise");
    test_support::keys(main.cx, "enter");
    assert_eq!(sent(&mut main.helm), []);
    assert!(!any_layer(main.cx) && !drawn(main.cx, "prompt-confirm"));

    main.cx.dispatch_action(FocusNextControl);
    assert_eq!(focused_id(main.cx).as_deref(), Some("session-card-restart"));
    main.cx.dispatch_action(FocusNextControl);
    assert_eq!(focused_id(main.cx).as_deref(), Some("session-card-replace"));

    test_support::keys(main.cx, "enter");
    assert_eq!(focused_id(main.cx).as_deref(), Some("prompt-confirm"));
    test_support::keys(main.cx, "escape");
    assert!(!drawn(main.cx, "prompt-confirm"));
    assert_eq!(sent(&mut main.helm), []);
    assert_eq!(focused_id(main.cx).as_deref(), Some("session-card-replace"));
}

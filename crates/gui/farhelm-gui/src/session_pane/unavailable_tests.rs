//! Tests of the unavailable card (`unavailable.rs`): what the pane mounts for a session whose host the helm cannot
//! reach, opened that way or gone that way while open; the terminals kept inert beneath the card and their notices held
//! back; where focus goes as the card comes and goes; the card as a keyboard scroll stop; the band that keeps the
//! last-known status and age; and the card's words for every way a host can be out of reach.
//!
//! The window tests run the real main view over the helm client's fake, whose connection hands out fake terminal
//! streams and records them, and read what the window drew from GPUI's driver record of the frame. The card's stop is
//! `session-unavailable`, its lines `session-unavailable-heading`, `-explanation` and `-detail`, and the band
//! `session-stale-band` with `session-stale-status` and `session-stale-age`. The first session of every snapshot is the
//! open one, on the helm fixture's host 2, which the helm names `buildbox`. The words are a pure function
//! ([`notice`]), tested without a window.

use std::cell::RefCell;
use std::rc::Rc;

use farhelm_helm_client::fake::terminal::{FakePhase, FakeTerminal};
use farhelm_proto::DetachCode;
use farhelm_proto::http::LOCAL_SUPERVISOR_NOT_RUNNING;
use farhelm_proto::tensorlake::LifecycleDirection;
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, ConnectionStage, HostStateView, HostView,
    KeyedEntry, RefreshView, SessionRow,
};
use std::time::Duration;

use gpui::{Entity, TestAppContext, px, size};
use serde_json::json;

use super::SessionPane;
use super::details::{AGE_ID, BAR_ID, DIRECTORY_ID, STATUS_ID, Status};
use super::unavailable::{
    BAND_AGE_ID, BAND_ID, BAND_STATUS_ID, Band, CARD_ID, DETAIL_ID, EXPLANATION_ID, HEADING_ID,
    Notice, notice,
};
use crate::actions::{DELETE_SESSION_CHORD, FocusPreviousControl};
use crate::host_text::HostText;
use crate::hosts::tests::LOCAL_DIAL_FAILURE;
use crate::main_view::MainView;
use crate::render_count;
use crate::session_list::menu_tests::{drawn, label_of, push, snapshot};
use crate::session_list::status::HostLookup;
use crate::session_list::tests::{Fixture, TEST_NOW, change, listing_meta, painted, session};
use crate::terminal::attached::INERT_SCRIM_ID;
use crate::terminal::set::STRIP_ID;
use crate::test_support::{self, MainWindow, focused_id, frame, started, terminal_focused};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// What the heading says for the fixture's unreachable host.
const WAITING: &str = "Session unavailable — waiting for supervisor";

/// The transport's words for the fixture's unreachable host.
const REFUSED: &str = "ssh: connect to host build-box.invalid port 22: Connection refused";

/// The open session `a` (running, last active two minutes before the test clock's start), stale when `stale` says so.
fn a(stale: bool) -> Fixture {
    let a = session("a").activity(TEST_NOW - 120);
    if stale { a.stale() } else { a }
}

/// The registry holding the fixtures' host 2 in `state`.
fn host_in(state: HostStateView) -> CollectionState {
    let view = crate::hosts::tests::host(2, "ssh", state);
    CollectionState {
        collection: Collection::Hosts,
        meta: test_support::meta(&[HostView::key(2).as_str()]),
        entries: vec![KeyedEntry {
            key: HostView::key(2),
            value: serde_json::to_value(view).unwrap(),
        }],
    }
}

/// The fixture's host as the helm reports it while it cannot be reached.
fn unreachable() -> HostStateView {
    HostStateView::Unreachable {
        cause: "transport-failure".into(),
        last_error: REFUSED.into(),
    }
}

/// `rows` (the first open) with the fixture's host in `state`.
fn fleet(rows: &[Fixture], state: HostStateView) -> Vec<CollectionState> {
    let rows: Vec<(Fixture, usize)> = rows.iter().cloned().map(|row| (row, 0)).collect();
    let mut snapshot = snapshot(&rows, false);
    snapshot.push(host_in(state));
    snapshot
}

/// The main view over the fake helm with `snapshot`, as [`test_support::open_main`] opens it, and its session pane.
fn open(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
) -> (MainWindow<'_>, Entity<SessionPane>) {
    let (mut helm, connection) = started(cx);
    let terminals: Rc<RefCell<Vec<FakeTerminal>>> = Rc::default();
    let record = terminals.clone();
    cx.update(|cx| {
        connection.update(cx, |connection, _| {
            connection.serve_terminals(move |attach| {
                let (terminal, handle, events) = FakeTerminal::open(&attach);
                record.borrow_mut().push(terminal);
                (handle, events)
            })
        })
    });
    helm.connect(snapshot);
    cx.run_until_parked();
    let (main, cx) = test_support::open_view(cx, MainView::new);
    let pane = cx.update(|_, cx| main.read(cx).session_pane().clone());
    (
        MainWindow {
            cx,
            helm,
            terminals,
        },
        pane,
    )
}

/// The phase of every stream the pane opened so far.
fn streams(main: &MainWindow) -> Vec<FakePhase> {
    main.terminals
        .borrow_mut()
        .iter_mut()
        .map(FakeTerminal::phase)
        .collect()
}

/// Whether the pane's terminal set is inert, as the set says and as each of its terminals says; `None` with no set.
fn inert(main: &mut MainWindow, pane: &Entity<SessionPane>) -> Option<(bool, Vec<bool>)> {
    main.cx.update(|_, cx| {
        let set = pane.update(cx, |pane, cx| pane.terminal_set(cx))?;
        let set = set.read(cx);
        let terminals = set
            .terminals()
            .map(|terminal| terminal.read(cx).is_inert())
            .collect();
        Some((set.is_inert(), terminals))
    })
}

/// Push session `a`'s row as `fixture`, with the fixture's host in `state`, under the listing `a`, `b`.
fn push_a(main: &mut MainWindow, fixture: Fixture, state: HostStateView) {
    main.helm.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: test_support::meta(&[HostView::key(2).as_str()]),
        upsert: host_in(state).entries,
        remove: Vec::new(),
    });
    push(&mut main.helm, &["a", "b"], &[(fixture, 0)], main.cx);
}

/// The host as the helm reports it while it cannot be reached, with a transport error long enough that the card
/// overflows its area in a window of the app's minimum height: what a card that has to scroll needs.
fn unreachable_at_length() -> HostStateView {
    HostStateView::Unreachable {
        cause: "transport-failure".into(),
        last_error: format!("{REFUSED}; ").repeat(40),
    }
}

/// The window's size in the scrolling tests: wide, and as short as the app's window may be (`MIN_WINDOW_SIZE`).
fn short_window(main: &mut MainWindow) {
    main.cx
        .simulate_resize(size(px(1000.), crate::bootstrap::MIN_WINDOW_SIZE.height));
    test_support::redraw(main.cx);
}

/// The unavailable card's scroll state, kept by the pane for the open session.
fn card_scroll(main: &mut MainWindow, pane: &Entity<SessionPane>) -> gpui::ScrollHandle {
    main.cx
        .update(|_, cx| {
            pane.update(cx, |pane, cx| {
                pane.mounted
                    .current(cx)
                    .as_ref()
                    .map(|mounted| mounted.scroll.clone())
            })
        })
        .expect("a session is open")
}

/// The id of what holds focus once the frames that move focus home have been drawn: a focus the frame no longer
/// tracks is reported lost on the next frame, and the coordinator may rest it on the root for one frame before home is
/// drawn and takes it.
fn settled_focus(main: &mut MainWindow) -> Option<String> {
    for _ in 0..3 {
        test_support::redraw(main.cx);
    }
    focused_id(main.cx)
}

/// The host as the helm reports it once it answers again.
fn connected() -> HostStateView {
    HostStateView::Connected {
        identity: None,
        build_version: "0.40.0".into(),
        old_version: false,
        newer_version: false,
        refresh: RefreshView::Pending,
    }
}

/// `fixture`'s row as the store holds it.
fn row_of(fixture: Fixture) -> SessionRow {
    serde_json::from_value(fixture.entry().value).unwrap()
}

// ---- The terminal or the card --------------------------------------------------------------------------------------

/// SPEC.md, "Session list": "A session opened while unreachable has no terminal or tab strip", and it "shows one calm
/// accent card naming the cause, with the helm's remedy and smaller diagnostics beneath it", its last-known status and
/// age "in exactly one band". An attach there would only meet a host that does not answer. Specified: with a stale a
/// open at startup, the pane opens no stream and draws no terminal and no strip; the card, named "Session unavailable",
/// sits between the top row and the details bar with the heading, the explanation and the host's evidence line; the
/// band names a's last-known Running and its age, and the details bar names neither; the card holds focus.
#[gpui::test]
fn a_session_opened_unreachable_shows_the_card_and_mounts_no_terminal(cx: &mut TestAppContext) {
    let (mut main, pane) = open(cx, fleet(&[a(true), session("b")], unreachable()));
    assert_eq!(streams(&main), [], "no stream");
    assert_eq!(inert(&mut main, &pane), None, "no terminals mounted");
    assert!(!drawn(main.cx, STRIP_ID), "no tab strip");
    assert_eq!(
        label_of(main.cx, CARD_ID).as_deref(),
        Some("Session unavailable")
    );
    assert_eq!(label_of(main.cx, HEADING_ID).as_deref(), Some(WAITING));
    assert_eq!(
        label_of(main.cx, EXPLANATION_ID).as_deref(),
        Some(
            "buildbox is not answering. The session keeps running there; this view reattaches by itself."
        )
    );
    assert_eq!(
        label_of(main.cx, DETAIL_ID),
        Some(format!("buildbox: {REFUSED}"))
    );
    let card = test_support::bounds(main.cx, CARD_ID);
    assert!(card.top() >= test_support::bounds(main.cx, BAND_ID).bottom());
    assert!(
        test_support::bounds(main.cx, BAND_ID).top()
            >= test_support::bounds(main.cx, "session-header").bottom()
    );
    assert!(card.bottom() <= test_support::bounds(main.cx, BAR_ID).top());
    assert_eq!(
        label_of(main.cx, BAND_STATUS_ID).as_deref(),
        Some("Running")
    );
    assert_eq!(
        label_of(main.cx, BAND_AGE_ID).as_deref(),
        Some("Last activity 2025-07-01 23:57:58 UTC")
    );
    assert!(
        !drawn(main.cx, STATUS_ID) && !drawn(main.cx, AGE_ID),
        "the bar leaves them to the band"
    );
    assert_eq!(focused_id(main.cx).as_deref(), Some(CARD_ID));
}

/// SPEC.md: "A terminal already open keeps its last screen and tab strip dimmed and inert beneath the card, clearly no
/// longer live", and "When the host returns, retained terminals retry promptly and automatically". The old card stood
/// over the panes it found and went once the session read fresh (`terminal-multihost.spec.ts`, "supervisor-outage";
/// "focus returns when replay precedes fresh detail"). Specified: with a live and its terminal focused, a push that
/// lists a stale keeps its one stream attached, makes the set and its terminal inert, draws the card over the terminal,
/// and focus, which the inert terminal no longer holds, settles on the card once the frames that send it home are
/// drawn, so a typed key reaches no program, and a press on the page's empty space leaves focus
/// on the card, which is home; a push that lists a reachable again makes the set live again on the same stream, takes
/// the card away, and gives focus back to the terminal without a click.
#[gpui::test]
fn a_session_whose_host_drops_away_keeps_its_terminal_inert_under_the_card(
    cx: &mut TestAppContext,
) {
    let (mut main, pane) = open(cx, fleet(&[a(false), session("b")], connected()));
    main.terminal_live();
    assert_eq!(
        inert(&mut main, &pane),
        Some((false, vec![false])),
        "the premise"
    );

    push_a(&mut main, a(true), unreachable());
    assert_eq!(streams(&main), [FakePhase::Attached], "the terminal stays");
    assert_eq!(inert(&mut main, &pane), Some((true, vec![true])));
    assert!(
        drawn(main.cx, INERT_SCRIM_ID) && drawn(main.cx, CARD_ID),
        "the card over the veiled terminal"
    );
    assert_eq!(settled_focus(&mut main).as_deref(), Some(CARD_ID));
    test_support::keys(main.cx, "x");
    assert_eq!(main.program_input(), 0, "nothing typed reaches the program");
    // A press on the page's empty space sends focus home, which must still be the card, not the inert terminal.
    test_support::click(main.cx, BAND_ID);
    assert_eq!(
        focused_id(main.cx).as_deref(),
        Some(CARD_ID),
        "home is the card"
    );

    push_a(&mut main, a(false), connected());
    assert_eq!(streams(&main), [FakePhase::Attached], "the same stream");
    assert_eq!(inert(&mut main, &pane), Some((false, vec![false])));
    assert!(!drawn(main.cx, CARD_ID));
    assert!(terminal_focused(main.cx), "the terminal takes focus back");
}

/// SPEC.md, "Where focus rests": "The terminal never takes focus from a control that holds it", and things the user did
/// not start never take focus (`terminal-multihost.spec.ts`: "focus left on another control is not taken", "recovery
/// does not steal focus"). The card moves focus only off the session's own terminal. Specified: with a live and focus
/// on the details bar's directory field (clicked, which focuses it on Linux), a push that lists a stale and then one
/// that lists it reachable leave focus on that field throughout.
#[gpui::test]
fn focus_elsewhere_stays_as_the_card_comes_and_goes(cx: &mut TestAppContext) {
    let (mut main, _) = open(cx, fleet(&[a(false), session("b")], connected()));
    main.terminal_live();
    test_support::click(main.cx, DIRECTORY_ID);
    assert_eq!(
        focused_id(main.cx).as_deref(),
        Some(DIRECTORY_ID),
        "the premise"
    );
    push_a(&mut main, a(true), unreachable());
    assert_eq!(focused_id(main.cx).as_deref(), Some(DIRECTORY_ID));
    push_a(&mut main, a(false), connected());
    assert_eq!(focused_id(main.cx).as_deref(), Some(DIRECTORY_ID));
}

/// The old stylesheet hid the terminal's own banner and reconnect surface under the card (`terminal-multihost.spec.ts`:
/// "the terminal's red banner hidden", "no detach banner ever showed"): a host that drops away drops the stream too,
/// and "Session unavailable — reconnecting" under a card that already says why would say it twice. Specified: with a
/// live, gone stale, and its stream dropped into a reconnect, the terminal is reconnecting (the premise) and nothing
/// paints its reconnect card's heading; once a is listed reachable again with the stream still down, the heading is
/// painted once.
#[gpui::test]
fn the_terminals_own_notices_stay_hidden_while_the_card_shows(cx: &mut TestAppContext) {
    const RECONNECTING: &str = "Session unavailable — reconnecting";
    let (mut main, pane) = open(cx, fleet(&[a(false), session("b")], connected()));
    main.terminal_live();
    push_a(&mut main, a(true), unreachable());
    main.terminals.borrow_mut()[0].detach(Some(DetachCode::Other), "host gone");
    test_support::settle(main.cx);
    let reconnecting = main.cx.update(|_, cx| {
        let set = pane.update(cx, |pane, cx| pane.terminal_set(cx))?;
        Some(set.read(cx).agent().read(cx).reconnecting())
    });
    assert_eq!(reconnecting, Some(true), "the premise");
    let count = |main: &mut MainWindow| {
        painted(&frame(main.cx))
            .iter()
            .filter(|text| *text == RECONNECTING)
            .count()
    };
    assert_eq!(count(&mut main), 0);
    push_a(&mut main, a(false), connected());
    assert_eq!(count(&mut main), 1);
}

/// A session opened unreachable has no terminal only until its host answers; then it is opened as any other, with its
/// terminal home. Specified: from a stale a (no stream), a push that lists a reachable opens one stream, attaching,
/// takes the card and the band away, and the new terminal takes focus from the card that held it.
#[gpui::test]
fn a_session_opened_unreachable_mounts_its_terminal_when_the_host_answers(cx: &mut TestAppContext) {
    let (mut main, _) = open(cx, fleet(&[a(true), session("b")], unreachable()));
    assert_eq!(streams(&main), [], "the premise");
    push_a(&mut main, a(false), connected());
    assert_eq!(streams(&main), [FakePhase::Attaching]);
    assert!(!drawn(main.cx, CARD_ID) && !drawn(main.cx, BAND_ID));
    assert!(terminal_focused(main.cx));
}

/// The inert terminal holds no focus, so focus that would come back to it goes home instead, and home is the card.
/// A layer opened from the terminal returns focus to the terminal when it closes, and the host can drop while it is
/// open, when nothing the pane does can move focus (it is inside the layer). Without that fallback focus would land on
/// a terminal that takes no key, and nothing would move it off. Specified: in a window of the app's minimum height,
/// with diagnostics long enough that the card scrolls, a live and its terminal focused (asserted, since that makes the
/// terminal the question's opener), the Delete shortcut opens the
/// header's Delete question (its focus restores to the terminal); a push lists a stale; the question's Cancel closes
/// it; focus settles on the card, and Page Down, which only the card's stop takes, scrolls the card's region.
#[gpui::test]
fn a_layer_closed_after_the_host_drops_leaves_focus_on_the_card(cx: &mut TestAppContext) {
    let (mut main, pane) = open(cx, fleet(&[a(false), session("b")], connected()));
    short_window(&mut main);
    main.terminal_live();
    assert!(
        terminal_focused(main.cx),
        "the premise: the terminal holds focus, so it is the question's opener"
    );
    test_support::keys(main.cx, DELETE_SESSION_CHORD);
    assert!(
        drawn(main.cx, "prompt-cancel"),
        "the premise: the question is open"
    );
    push_a(&mut main, a(true), unreachable_at_length());
    assert!(
        drawn(main.cx, "prompt-cancel"),
        "the premise: the question stays open"
    );
    test_support::click(main.cx, "prompt-cancel");
    assert_eq!(settled_focus(&mut main).as_deref(), Some(CARD_ID));
    let scroll = card_scroll(&mut main, &pane);
    assert!(
        scroll.max_offset().y > px(0.),
        "the premise: the card can scroll"
    );
    test_support::keys(main.cx, "pagedown");
    assert!(scroll.offset().y < px(0.), "Page Down reached the card");
}

/// A tab the set learns about while the card shows is made inert at once, and the set's notice of it must not make
/// its terminals home again: the pane's home rule is the whole session's, card included. Specified: with a live and its
/// terminal focused, a push lists a stale (focus settles on the card), then a push lists it stale with a tab; the set
/// and both its terminals are inert, focus is still on the card, and a press on the page's empty space, which sends
/// focus home, leaves it on the card.
#[gpui::test]
fn a_tab_that_arrives_under_the_card_leaves_focus_on_the_card(cx: &mut TestAppContext) {
    let (mut main, pane) = open(cx, fleet(&[a(false), session("b")], connected()));
    main.terminal_live();
    push_a(&mut main, a(true), unreachable());
    assert_eq!(
        settled_focus(&mut main).as_deref(),
        Some(CARD_ID),
        "the premise"
    );
    push(&mut main.helm, &["a", "b"], &[(a(true), 1)], main.cx);
    assert_eq!(inert(&mut main, &pane), Some((true, vec![true, true])));
    assert_eq!(settled_focus(&mut main).as_deref(), Some(CARD_ID));
    test_support::click(main.cx, BAND_ID);
    assert_eq!(settled_focus(&mut main).as_deref(), Some(CARD_ID));
}

/// SPEC.md, "Where focus rests": "Choosing a session, including the one already selected, focuses" what is home, and
/// for a session on an unreachable host that is the card. Specified: with the reachable b open and its terminal
/// focused, a click on stale a's row opens a with no terminal and focus on the card, not on the row that was clicked.
#[gpui::test]
fn choosing_an_unreachable_session_focuses_its_card(cx: &mut TestAppContext) {
    let (main, _) = open(cx, fleet(&[session("b"), a(true)], connected()));
    test_support::click(main.cx, "session-row-a");
    assert!(
        drawn(main.cx, CARD_ID),
        "the premise: a is open on its card"
    );
    assert_eq!(focused_id(main.cx).as_deref(), Some(CARD_ID));
}

/// The follower tells a stale row from no row at all, because the two differ in what the pane does with the terminals:
/// a stale row shows the card over them, inert; no row (the moment between the helm removing a Replace's source and
/// its reply, while the selection stays on the source) shows no card, so the terminals must not stay inert under
/// nothing. Neither is reachable, so a follower that only knew reachability would not notice the step from one to the
/// other. Specified: with a live, a push lists it stale (the set inert); the header's Replace is confirmed and left
/// unanswered; a push removes a and lists its replacement n; a stays selected (the premise), no card is drawn, and the
/// set is live again.
#[gpui::test]
fn a_stale_row_that_goes_away_leaves_no_inert_terminal_under_nothing(cx: &mut TestAppContext) {
    let (mut main, pane) = open(cx, fleet(&[a(false), session("b")], connected()));
    main.terminal_live();
    push_a(&mut main, a(true), unreachable());
    assert_eq!(
        inert(&mut main, &pane),
        Some((true, vec![true])),
        "the premise"
    );
    test_support::click(main.cx, "session-header-replace");
    test_support::click(main.cx, "prompt-confirm");
    for command in main.helm.commands() {
        if !command.path.ends_with("/replace") {
            main.helm.reply(command.id, 200, None);
        }
    }
    main.helm.push(change(
        listing_meta(&["n", "b"], 2, false),
        &[session("n")],
        &["a"],
    ));
    test_support::settle(main.cx);
    assert_eq!(
        main.cx
            .update(|_, cx| crate::local_state::selected(cx))
            .as_deref(),
        Some("a"),
        "the premise: the source stays open"
    );
    assert!(!drawn(main.cx, CARD_ID));
    assert_eq!(inert(&mut main, &pane), Some((false, vec![false])));
}

/// The card's heading carries the old card's spinner, which turns while the card shows, as the pane's other spinners
/// do: through the pane's one spinner, which redraws the pane at the ticker's steps. Nothing else here moves: the list
/// is not drawn, a stale row's mark holds still, the bar leaves the stale session's status out, and the clocks' 30
/// seconds are far away. Specified: in an active window drawing the pane alone, with a stale idle session open on its
/// card, a second redraws the pane at least once; the same window made inactive, which stops the spinner, then
/// redraws it not once in a second, so the redraws were the spinner's.
#[gpui::test]
fn the_cards_spinner_turns(cx: &mut TestAppContext) {
    use super::details_tests::PaneOnly;
    let (mut fake, _connection) = started(cx);
    fake.connect(fleet(
        &[a(true).status(json!({"state": "idle"}))],
        unreachable(),
    ));
    cx.run_until_parked();
    let (_view, cx) = test_support::open_view(cx, PaneOnly::new);
    assert!(drawn(cx, CARD_ID), "the premise: the card shows");
    cx.update(|_, cx| render_count::reset(cx));
    test_support::advance(cx, Duration::from_secs(1));
    assert!(cx.update(|_, cx| render_count::count("session-pane", cx)) > 0);
    // The baseline: the same window inactive, where the spinner stands still, draws the pane not once in a second.
    cx.deactivate_window();
    test_support::settle(cx);
    cx.update(|_, cx| render_count::reset(cx));
    test_support::advance(cx, Duration::from_secs(1));
    assert_eq!(
        cx.update(|_, cx| render_count::count("session-pane", cx)),
        0,
        "the baseline: nothing else redraws the pane"
    );
}

/// The band's age moves on by itself, through the ticker's 30-second clock, which the pane holds for the band as it
/// does for the details bar's age; the bar leaves a stale session's age out, so the band is the only age the pane
/// draws then. Specified: in an inactive window drawing the pane alone (so the card's spinner stands still and the
/// list's rows hold no clock), with a stale idle session 59 minutes 40 seconds old, the band paints `59m`; 29 seconds
/// redraw the pane not once; the 30th redraws it (counted before any frame the test draws), and the band then paints
/// `1h`.
#[gpui::test]
fn the_bands_age_moves_on_with_the_shared_clock(cx: &mut TestAppContext) {
    use super::details_tests::{PaneOnly, painted_within};
    let row = a(true)
        .status(json!({"state": "idle"}))
        .activity(TEST_NOW - (59 * 60 + 40));
    let (mut fake, _connection) = started(cx);
    fake.connect(fleet(&[row], unreachable()));
    cx.run_until_parked();
    let (_view, cx) = test_support::open_view(cx, PaneOnly::new);
    assert_eq!(
        painted_within(&frame(cx), BAND_AGE_ID),
        ["59m"],
        "the premise"
    );
    cx.deactivate_window();
    test_support::settle(cx);
    cx.update(|_, cx| render_count::reset(cx));
    test_support::advance(cx, Duration::from_secs(29));
    assert_eq!(
        cx.update(|_, cx| render_count::count("session-pane", cx)),
        0,
        "the premise: nothing else redraws the pane"
    );
    test_support::advance(cx, Duration::from_secs(1));
    assert!(
        cx.update(|_, cx| render_count::count("session-pane", cx)) > 0,
        "the clock redrew the pane"
    );
    assert_eq!(painted_within(&frame(cx), BAND_AGE_ID), ["1h"]);
}

/// SPEC.md: "A rebooted session still shows its interrupted card", and stale beats interrupted only while the host is
/// away. The retained terminal belonged to a run the reboot ended, so it must go when the host answers with the
/// session interrupted, and focus must pass from the unavailable card to the interrupted one without falling through:
/// the two cards share one focus handle for that. Specified: with a live and its terminal focused, a push lists a stale
/// (focus settles on the card, on the pane's card handle); a push then lists a reachable and interrupted: the one
/// stream is closed, no terminals are mounted, the interrupted card shows and the unavailable card does not, and focus
/// is on the interrupted card, still on the same handle.
#[gpui::test]
fn a_host_back_with_the_session_interrupted_hands_focus_to_the_interrupted_card(
    cx: &mut TestAppContext,
) {
    let (mut main, pane) = open(cx, fleet(&[a(false), session("b")], connected()));
    main.terminal_live();
    push_a(&mut main, a(true), unreachable());
    assert_eq!(
        settled_focus(&mut main).as_deref(),
        Some(CARD_ID),
        "the premise"
    );
    let handle = main
        .cx
        .update(|_, cx| {
            pane.update(cx, |pane, cx| {
                pane.mounted
                    .current(cx)
                    .as_ref()
                    .map(|mounted| mounted.card_focus.clone())
            })
        })
        .expect("a session is open");
    assert!(
        test_support::focused(main.cx, &handle),
        "the premise: the cards' handle"
    );

    push_a(
        &mut main,
        a(false).status(json!({"state": "interrupted"})),
        connected(),
    );
    assert_eq!(
        streams(&main),
        [FakePhase::Closed],
        "the retained terminal goes"
    );
    assert_eq!(inert(&mut main, &pane), None, "no terminals mounted");
    assert!(drawn(main.cx, "session-interrupted-card") && !drawn(main.cx, CARD_ID));
    assert_eq!(
        settled_focus(&mut main).as_deref(),
        Some("session-interrupted-card")
    );
    assert!(test_support::focused(main.cx, &handle), "the same handle");
}

/// The card holds focus but no action of its own, so Escape there must do nothing, and a header question asked from
/// the card returns focus home on Escape, which is the card while it shows (`returns_home_on_escape`), a path through
/// the layer manager that Cancel does not take. Specified: with a stale a open and its card focused, Escape opens and
/// closes nothing and leaves focus on the card; the Delete shortcut asks the header's question, and Escape closes it
/// with focus back on the card.
#[gpui::test]
fn escape_at_the_card_does_nothing_and_a_question_escaped_returns_to_it(cx: &mut TestAppContext) {
    let (mut main, _) = open(cx, fleet(&[a(true), session("b")], unreachable()));
    assert_eq!(
        settled_focus(&mut main).as_deref(),
        Some(CARD_ID),
        "the premise"
    );
    test_support::keys(main.cx, "escape");
    assert!(!test_support::any_open(main.cx));
    assert_eq!(settled_focus(&mut main).as_deref(), Some(CARD_ID));
    test_support::keys(main.cx, DELETE_SESSION_CHORD);
    assert!(
        drawn(main.cx, "prompt-cancel"),
        "the premise: the question is open"
    );
    test_support::keys(main.cx, "escape");
    assert!(!drawn(main.cx, "prompt-cancel"));
    assert_eq!(settled_focus(&mut main).as_deref(), Some(CARD_ID));
}

/// A reset throws away everything the window held from the helm but the selection (SPEC.md, "Losing the connection to
/// the helm"), so the inert terminals of the session that went stale go with the old UI session, and the new one opens
/// the session as any session opened while its host is away: with no terminals. Specified: with a live, then stale
/// (the set inert), the channel breaks and reconnects with a snapshot that lists a stale; a is still selected, the old
/// stream is closed and no new one opens, no terminals are mounted, and the card shows and holds focus.
#[gpui::test]
fn a_reset_while_stale_mounts_no_terminal(cx: &mut TestAppContext) {
    let (mut main, pane) = open(cx, fleet(&[a(false), session("b")], connected()));
    main.terminal_live();
    push_a(&mut main, a(true), unreachable());
    assert_eq!(
        inert(&mut main, &pane),
        Some((true, vec![true])),
        "the premise"
    );
    main.helm.break_channel();
    test_support::settle(main.cx);
    main.helm
        .connect(fleet(&[a(true), session("b")], unreachable()));
    test_support::settle(main.cx);
    assert_eq!(
        main.cx
            .update(|_, cx| crate::local_state::selected(cx))
            .as_deref(),
        Some("a")
    );
    assert_eq!(streams(&main), [FakePhase::Closed]);
    assert_eq!(inert(&mut main, &pane), None);
    assert!(drawn(main.cx, CARD_ID));
    assert_eq!(settled_focus(&mut main).as_deref(), Some(CARD_ID));
}

// ---- The card as a scroll stop -------------------------------------------------------------------------------------

/// The old card was a keyboard stop so that "a short terminal area never makes reading a long remedy depend on a
/// pointer" (`terminal-multihost.spec.ts`, "stale-session-metadata-view": Shift+Tab from the first copy field reaches
/// the notice, and End and Home scroll it). Specified: in a window of the app's minimum height, with diagnostics long
/// enough that the card overflows its area (the premise, asserted), and focus on the details bar's directory field (its
/// premise asserted too), Shift+Tab reaches the card; End scrolls its region to the end and Home back to the top.
#[gpui::test]
fn the_card_is_a_scroll_stop_that_tab_reaches(cx: &mut TestAppContext) {
    let (mut main, pane) = open(cx, fleet(&[a(true), session("b")], unreachable_at_length()));
    short_window(&mut main);
    let scroll = card_scroll(&mut main, &pane);
    assert!(
        scroll.max_offset().y > px(0.),
        "the premise: the card overflows its area"
    );
    test_support::click(main.cx, DIRECTORY_ID);
    assert_eq!(
        focused_id(main.cx).as_deref(),
        Some(DIRECTORY_ID),
        "the premise: focus is on the directory field"
    );
    main.cx.dispatch_action(FocusPreviousControl);
    assert_eq!(focused_id(main.cx).as_deref(), Some(CARD_ID));
    test_support::keys(main.cx, "end");
    assert_eq!(scroll.offset().y, -scroll.max_offset().y);
    test_support::keys(main.cx, "home");
    assert_eq!(scroll.offset().y, px(0.));
}

// ---- What the card says --------------------------------------------------------------------------------------------

/// A host in `state`, as the registry lists it.
fn known(state: HostStateView) -> HostView {
    crate::hosts::tests::host(2, "ssh", state)
}

/// The notice's three lines as plain text: the heading, the explanation and the diagnostics.
fn lines(notice: &Notice) -> (String, String, Option<String>) {
    (
        notice.heading(),
        notice.explanation.accessible_name().to_string(),
        notice
            .detail
            .as_ref()
            .map(|detail| detail.accessible_name().to_string()),
    )
}

/// The card's words for `state` on the host named `buildbox`, with a duplicate's twin named `other-box`.
fn words_for(state: HostStateView) -> (String, String, Option<String>) {
    let host = known(state);
    lines(&notice("buildbox", HostLookup::Known(&host), None, |_| {
        Some(HostText::from_host("other-box"))
    }))
}

/// The card's words are the only place a session on an unreachable host says why and what to do, and the old card's are
/// pinned case for case (`hosts.rs`'s `stale_session_notice` at the reference commit), with the remedy and evidence the
/// Hosts panel shows for the host. Specified: an unreachable host that is not answering, the local host with no
/// supervisor (its remedy, to reopen the Farhelm app, as the explanation and the helm's dial failure as the
/// diagnostics), a version skew (the helm's remedy verbatim), a duplicate (naming its twin), a connected host whose
/// listing is catching up, a suspended, suspending and stopped environment, an unread host list, one not read yet, and
/// a removed host, each with its heading, explanation and diagnostics.
#[test]
fn the_cards_words_name_the_cause_the_remedy_and_the_evidence() {
    assert_eq!(
        words_for(unreachable()),
        (
            WAITING.to_owned(),
            "buildbox is not answering. The session keeps running there; this view reattaches by itself.".to_owned(),
            Some(format!("buildbox: {REFUSED}")),
        )
    );
    let local = HostStateView::Unreachable {
        cause: LOCAL_SUPERVISOR_NOT_RUNNING.into(),
        last_error: LOCAL_DIAL_FAILURE.into(),
    };
    assert_eq!(
        words_for(local),
        (
            WAITING.to_owned(),
            "Quit Farhelm and open it again; it starts this machine's supervisor when it opens"
                .to_owned(),
            Some(format!("buildbox: {LOCAL_DIAL_FAILURE}")),
        )
    );
    let connecting = HostStateView::Connecting {
        attempt: 3,
        stage: ConnectionStage::ConnectingToHost,
        last_error: None,
    };
    assert_eq!(
        words_for(connecting).2.as_deref(),
        Some("buildbox: Attempt 3: connecting to the host…")
    );
    let skew = HostStateView::VersionSkew {
        peer_protocol: 7,
        peer_build: "0.39.0".into(),
        our_protocol: 8,
        our_build: "0.40.0".into(),
        remediation: "Update the host to this helm's build".into(),
    };
    let (heading, explanation, _) = words_for(skew);
    assert_eq!(heading, "Session unavailable — host needs update");
    assert_eq!(explanation, "Update the host to this helm's build");
    let duplicate = HostStateView::Duplicate {
        twin: 5,
        identity: "install-1".into(),
    };
    let (heading, explanation, _) = words_for(duplicate);
    assert_eq!(heading, "Session unavailable — duplicate host");
    assert_eq!(
        explanation,
        "Remove other-box or change this entry's destination, then press Retry"
    );
    assert_eq!(
        words_for(connected()),
        (
            "Session unavailable — refreshing session".to_owned(),
            "Refreshing this session's state.".to_owned(),
            None,
        )
    );
    let suspended = HostStateView::Suspended {
        observed_at: 0,
        suspended_by_farhelm_at: None,
        resume_failed: None,
    };
    let (heading, explanation, detail) = words_for(suspended);
    assert_eq!(heading, "Session unavailable — host suspended");
    assert_eq!(
        explanation,
        "buildbox is suspended; this session continues when it resumes."
    );
    assert!(detail.is_some_and(|detail| detail.starts_with("buildbox: Suspension noticed at ")));
    let suspending = HostStateView::TensorlakeChanging {
        starting: false,
        direction: LifecycleDirection::Suspending,
        requested_at: 0,
    };
    assert_eq!(
        words_for(suspending).1,
        "buildbox is suspending; this session continues when the host resumes."
    );
    let stopped = HostStateView::Stopped {
        start_failed: None,
        waiting_for_place: false,
    };
    assert_eq!(
        words_for(stopped),
        (
            "Session unavailable — environment stopped".to_owned(),
            "The environment is stopped. Resume starts it again; its files remain.".to_owned(),
            None,
        )
    );
    let failed = notice(
        "buildbox",
        HostLookup::Failed,
        Some("the helm's registry is locked"),
        |_| None,
    );
    assert_eq!(
        lines(&failed),
        (
            "Session unavailable — host state unavailable".to_owned(),
            "The helm's host state could not be refreshed.".to_owned(),
            Some("the helm's registry is locked".to_owned()),
        )
    );
    assert_eq!(
        lines(&notice("buildbox", HostLookup::Pending, None, |_| None)),
        (
            "Session unavailable — checking host".to_owned(),
            "Checking buildbox…".to_owned(),
            None
        )
    );
    assert_eq!(
        lines(&notice("buildbox", HostLookup::Absent, None, |_| None)),
        (
            "Session unavailable — host removed".to_owned(),
            "This host is no longer registered with this helm. Re-adding the destination would reach it again."
                .to_owned(),
            Some("buildbox".to_owned()),
        )
    );
}

/// A session the helm sent without a host name (one from before host names) is still named, in the old card's words,
/// rather than read as a sentence that starts with nothing; and a host's name is a host value, escaped where the
/// screen shows it. Specified: with no name, an unreachable host's explanation starts "This session's host", the
/// diagnostics "This session's host: ", and a host not read yet is "Checking this session's host…"; a name holding a
/// right-to-left override is named with it escaped.
#[test]
fn a_session_without_a_host_name_is_still_named_and_a_name_is_escaped() {
    let host = known(unreachable());
    let unnamed = lines(&notice("", HostLookup::Known(&host), None, |_| None));
    assert!(
        unnamed
            .1
            .starts_with("This session's host is not answering.")
    );
    assert_eq!(unnamed.2, Some(format!("This session's host: {REFUSED}")));
    assert_eq!(
        lines(&notice("", HostLookup::Pending, None, |_| None)).1,
        "Checking this session's host…"
    );
    let spoofed = lines(&notice(
        "evil\u{202e}box",
        HostLookup::Pending,
        None,
        |_| None,
    ));
    assert_eq!(spoofed.1, "Checking evil<U+202E>box…");
}

// ---- The band ------------------------------------------------------------------------------------------------------

/// SPEC.md: the last-known status and age go "in exactly one band", and the details bar leaves them out while the card
/// shows. What the band holds is decided from the row alone. Specified: a stale running row has the running mark and
/// its age; a stale exited row its ended words and age; a stale row nobody classified only its age; a reachable row no
/// band at all.
#[test]
fn the_band_keeps_the_last_known_status_and_age_of_a_stale_session_only() {
    let now = Some(TEST_NOW);
    let running = Band::of(&row_of(a(true)), now).expect("a band");
    assert_eq!(
        running.status,
        Some(Status::Live(crate::session_list::status::Live::Running))
    );
    assert_eq!(running.age.map(|age| age.short).as_deref(), Some("2m"));
    let exited = Band::of(
        &row_of(a(true).status(json!({"state": "exited", "exit_code": 0}))),
        now,
    )
    .expect("a band");
    let Some(Status::Ended(ended)) = exited.status else {
        panic!("an ended status");
    };
    assert_eq!(ended.words, "Exited (code 0)");
    let unknown =
        Band::of(&row_of(a(true).status(json!({"state": "unknown"}))), now).expect("a band");
    assert!(unknown.status.is_none() && unknown.age.is_some());
    assert_eq!(Band::of(&row_of(a(false)), now), None);
}

//! Tests of the approval region: where it places itself, which request opens, what it does as requests come and go,
//! and that it answers, over an open dialog too, without taking focus from what the user is doing.
//!
//! They matter because the region is the only way the user sees an agent's request: a card placed off the pane, a
//! request that never opens, a card that takes focus from a terminal (so Enter typed there answers it), or a card that
//! cannot be answered while a dialog is open would each leave an agent waiting nine minutes or answer something the
//! user did not mean to. Placement is a pure function, tested on numbers; the rest runs in a window over the helm
//! client's fake, with the region installed as the main view installs it and the shared harness's focus and overlay
//! checks after every step. The region's look waits for screenshot scenes of the running app.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::approvals::{ApprovalAction, ApprovalSession, PendingApproval};
use farhelm_proto::ui_channel::{Collection, CollectionChanges, CollectionState, KeyedEntry};
use gpui::{
    Bounds, Context, FocusHandle, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, div, point, px, size,
};
use serde_json::json;

use super::answers::ARM_DELAY;
use super::region::{self, Place, place};
use crate::layers::Anchor;
use crate::notices::{self, Notice};
use crate::root;
use crate::test_support::{
    self, any_open, button, click, focused, frame, keys, label, meta, one_command, settle, started,
};

// ---- Placement -------------------------------------------------------------------------------------------------------

/// A pane at (300, 0), 1000 by 800.
fn pane() -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(300.), px(0.)), size(px(1000.), px(800.)))
}

/// The old layout script's placement: the card's width capped at 660 and centered in the pane, 12 pixels below the
/// terminal area's top (the pane's top with no session), at most three quarters of the pane and the room left below.
/// Specified: in a 1000-pixel pane the region is 660 wide at x 470; below a terminal area at 80 it starts at 92 with a
/// cap of 600 (three quarters of 800); with no session it starts at 12; in a 500-pixel pane it is 476 wide.
#[test]
fn the_region_is_centered_below_the_terminal_area_and_capped() {
    assert_eq!(
        place(pane(), Some(px(80.)), px(800.), px(200.)),
        Place {
            left: px(470.),
            width: px(660.),
            top: px(92.),
            cap: px(600.),
            scrolls: false
        }
    );
    assert_eq!(place(pane(), None, px(800.), px(200.)).top, px(12.));
    let narrow = Bounds::new(point(px(300.), px(0.)), size(px(500.), px(800.)));
    let placed = place(narrow, None, px(800.), px(200.));
    assert_eq!((placed.width, placed.left), (px(476.), px(312.)));
}

/// SPEC.md: the card's header and answer bar stay visible within the cap; if session chrome leaves less than they
/// need, the region uses the whole viewport and may cover that chrome; below that minimum the region scrolls as a
/// whole. Specified: below a terminal area at 700 in an 800-pixel pane the cap is 76; fixed parts of 200 then take the
/// whole 800-pixel window from its top, unscrolled; fixed parts of 900 scroll it.
#[test]
fn a_short_pane_gives_the_region_the_window_and_a_tiny_one_scrolls_it() {
    assert_eq!(
        place(pane(), Some(px(700.)), px(800.), px(50.)).cap,
        px(76.)
    );
    let tall = place(pane(), Some(px(700.)), px(800.), px(200.));
    assert_eq!(
        (tall.top, tall.cap, tall.scrolls),
        (px(0.), px(800.), false)
    );
    let tiny = place(pane(), Some(px(700.)), px(800.), px(900.));
    assert!(
        tiny.scrolls,
        "even the window is shorter than the fixed parts"
    );
}

// ---- The window --------------------------------------------------------------------------------------------------------

/// A waiting stop request `id` from host `host`.
fn stop(id: &str, host: &str) -> PendingApproval {
    PendingApproval {
        id: id.to_owned(),
        host_id: 1,
        host_name: host.to_owned(),
        session: ApprovalSession {
            id: "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:asker".to_owned(),
            title: None,
            host_name: None,
        },
        action: ApprovalAction::Stop {
            target: ApprovalSession {
                id: "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:target".to_owned(),
                title: None,
                host_name: None,
            },
        },
        own_host_session_action: false,
        expires_at_ms: 1,
    }
}

/// The approvals collection holding `approvals`, oldest first.
fn waiting(approvals: &[PendingApproval]) -> CollectionState {
    let ids: Vec<&str> = approvals
        .iter()
        .map(|approval| approval.id.as_str())
        .collect();
    CollectionState {
        collection: Collection::Approvals,
        meta: meta(&ids),
        entries: approvals
            .iter()
            .map(|approval| KeyedEntry {
                key: approval.id.clone(),
                value: serde_json::to_value(approval).expect("an approval encodes"),
            })
            .collect(),
    }
}

/// The pushed list becomes `approvals`, oldest first.
fn push(helm: &mut FakeHelm, approvals: &[PendingApproval], gone: &[&str]) {
    let ids: Vec<&str> = approvals
        .iter()
        .map(|approval| approval.id.as_str())
        .collect();
    helm.push(CollectionChanges {
        collection: Collection::Approvals,
        meta: meta(&ids),
        upsert: waiting(approvals).entries,
        remove: gone.iter().map(|id| (*id).to_owned()).collect(),
    });
}

/// The main view's shape for the region: a pane the region centers in, holding a control standing in for a focused
/// terminal.
struct Shell {
    root: root::Attachment,
    terminal: FocusHandle,
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = Anchor::for_element("test-pane-anchor", window, cx);
        region::set_pane(pane.clone(), cx);
        let content = div()
            .size_full()
            .relative()
            .child(button("terminal", &self.terminal))
            .child(pane.track());
        root::wrap(content, &self.root, window, cx)
    }
}

/// The app over the fake helm with `approvals` waiting, a window with the region installed as the main view installs
/// it, focus on the stand-in terminal, and the answer bar's pause run out.
fn open<'a>(
    cx: &'a mut TestAppContext,
    approvals: &[PendingApproval],
) -> (FakeHelm, &'a mut VisualTestContext, FocusHandle) {
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![waiting(approvals)]);
    cx.run_until_parked();
    let terminal = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let held = terminal.clone();
    let (_, window) = test_support::open_view(cx, move |window, cx| {
        let root = root::attach(window, cx);
        region::install(window, cx);
        Shell {
            root,
            terminal: held,
        }
    });
    window.simulate_resize(size(px(1200.), px(900.)));
    window.update(|window, cx| {
        let mark = crate::focus::Mark::take(cx);
        crate::focus::focus_control(&terminal, &mark, window, cx);
    });
    settle(window);
    assert!(
        focused(window, &terminal),
        "premise: the stand-in terminal holds focus"
    );
    window.executor().advance_clock(ARM_DELAY);
    settle(window);
    (helm, window, terminal)
}

/// Whether the region draws anything.
fn drawn(cx: &mut VisualTestContext) -> bool {
    frame(cx)
        .iter()
        .any(|node| node.element_id.as_deref() == Some("approval-region"))
}

/// The id of the request whose card is expanded.
fn expanded(cx: &mut VisualTestContext) -> Option<String> {
    frame(cx).into_iter().find_map(|node| {
        node.element_id
            .as_deref()?
            .strip_prefix("approval-")?
            .strip_suffix("-card")
            .map(str::to_owned)
    })
}

// ---- What shows ------------------------------------------------------------------------------------------------------

/// An empty region covers nothing: with no request waiting and no notice, it draws nothing at all (the slot's
/// contract). Specified: nothing waiting, no region; one request, the region with its card.
#[gpui::test]
fn the_region_draws_only_while_a_request_waits(cx: &mut TestAppContext) {
    let (mut helm, cx, _terminal) = open(cx, &[]);
    assert!(!drawn(cx), "nothing waits: nothing drawn");
    push(&mut helm, &[stop("a", "box")], &[]);
    settle(cx);
    assert!(drawn(cx), "a request waits: the region shows it");
    assert_eq!(expanded(cx).as_deref(), Some("a"));
}

/// SPEC.md: the oldest waiting request opens first; a selected header stays selected until that request is answered or
/// expires, then the oldest remaining one opens; the others are listed below as headers. Specified: with a and b
/// waiting, a opens and b is a header named for its action and host; choosing b opens it; a newer request c arriving
/// leaves b open; b ending opens a, the oldest.
#[gpui::test]
fn the_oldest_opens_and_a_chosen_header_stays_until_its_request_ends(cx: &mut TestAppContext) {
    let (mut helm, cx, _terminal) = open(cx, &[stop("a", "box"), stop("b", "other")]);
    assert_eq!(expanded(cx).as_deref(), Some("a"));
    assert_eq!(
        label(cx, "approval-b-waiting").as_deref(),
        Some("Stop a session's agent and close its terminal tabs, from host other")
    );
    click(cx, "approval-b-waiting");
    assert_eq!(
        expanded(cx).as_deref(),
        Some("b"),
        "the chosen header opens"
    );
    push(
        &mut helm,
        &[stop("a", "box"), stop("b", "other"), stop("c", "box")],
        &[],
    );
    settle(cx);
    assert_eq!(
        expanded(cx).as_deref(),
        Some("b"),
        "a new arrival leaves the choice"
    );
    push(&mut helm, &[stop("a", "box"), stop("c", "box")], &["b"]);
    settle(cx);
    assert_eq!(
        expanded(cx).as_deref(),
        Some("a"),
        "the chosen request ended: the oldest opens"
    );
}

// ---- Focus and answers -------------------------------------------------------------------------------------------------

/// SPEC.md, "Keyboard focus": a request arriving moves no focus, and Enter typed into a focused terminal keeps going to
/// it, never answering a card. Specified: with focus on the stand-in terminal, a request arrives; focus stays there,
/// and Enter sends nothing.
#[gpui::test]
fn an_arriving_request_takes_no_focus_and_enter_answers_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx, terminal) = open(cx, &[]);
    push(&mut helm, &[stop("a", "box")], &[]);
    settle(cx);
    assert!(drawn(cx), "premise: the card is up");
    assert!(focused(cx, &terminal), "focus stayed on the terminal");
    // Past the answer bar's pause, so Enter would answer if focus had gone to a card's button.
    cx.executor().advance_clock(ARM_DELAY);
    settle(cx);
    keys(cx, "enter");
    assert!(helm.commands().is_empty(), "Enter answered nothing");
}

/// SPEC.md: answer buttons pause for 700 ms whenever the waiting list or the expanded request changes. Specified: a
/// press on Deny right after the list changed sends nothing; once 700 ms have passed, it sends the Deny.
#[gpui::test]
fn the_answers_pause_after_the_list_changes(cx: &mut TestAppContext) {
    let (mut helm, cx, _terminal) = open(cx, &[stop("a", "box")]);
    push(&mut helm, &[stop("a", "box"), stop("b", "box")], &[]);
    settle(cx);
    click(cx, "approval-a-answer-deny");
    assert!(helm.commands().is_empty(), "paused: nothing sent");
    cx.executor().advance_clock(ARM_DELAY);
    settle(cx);
    click(cx, "approval-a-answer-deny");
    let (_, body) = one_command(&mut helm, Method::Post, "/api/approvals/a");
    assert_eq!(body, json!({ "answer": "deny" }));
}

/// SPEC.md: cards remain answerable over open dialogs. Specified: with a dialog open, Deny on the card sends the answer
/// and the dialog stays open.
#[gpui::test]
fn a_card_is_answerable_over_an_open_dialog(cx: &mut TestAppContext) {
    let (mut helm, cx, _terminal) = open(cx, &[stop("a", "box")]);
    let inside = cx.update(|_, cx| cx.focus_handle().tab_stop(true));
    test_support::open_dialog(cx, vec![("dialog-control", inside.clone())], &inside);
    assert!(any_open(cx), "premise: a dialog is open");
    click(cx, "approval-a-answer-deny");
    let (_, body) = one_command(&mut helm, Method::Post, "/api/approvals/a");
    assert_eq!(body, json!({ "answer": "deny" }));
    assert!(any_open(cx), "the dialog stays open");
}

// ---- The late notice ---------------------------------------------------------------------------------------------------

/// SPEC.md: an answer that arrives after its request stopped waiting says so, in the region, above the card, until
/// dismissed. Specified: with a request waiting, the posted late-answer notice is drawn with its words above that
/// request's card; with nothing waiting it still draws the region; Dismiss withdraws it and the region draws nothing
/// again.
#[gpui::test]
fn the_late_notice_shows_in_the_region_until_dismissed(cx: &mut TestAppContext) {
    let (mut helm, cx, _terminal) = open(cx, &[stop("a", "box")]);
    cx.update(|_, cx| notices::post(Notice::LateAnswer { always: false }, cx));
    settle(cx);
    assert_eq!(
        label(cx, "approval-late"),
        Some(Notice::LateAnswer { always: false }.text().to_string())
    );
    let top = |cx: &mut VisualTestContext, id: &str| {
        frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .map(|node| node.bounds.origin.y)
            .unwrap_or_else(|| panic!("{id} is drawn"))
    };
    assert!(
        top(cx, "approval-late") < top(cx, "approval-a-card"),
        "the notice sits above the card"
    );
    push(&mut helm, &[], &["a"]);
    settle(cx);
    assert!(drawn(cx), "the notice alone still draws the region");
    click(cx, "approval-late-dismiss");
    assert!(!drawn(cx), "dismissed: nothing left to draw");
}

/// SPEC.md: if session chrome leaves less than the card's header and answer bar need, the region uses the whole
/// viewport. What those need is measured as they are drawn, so the first frame of a new card cannot know it: the region
/// must draw again by itself once it does, not wait for something else to redraw the window. Specified: in a window
/// 150 pixels high (a cap of 112, less than a card's header, From line and answer bar), a request arriving puts the region at the window's top (the fallback) once the window settles,
/// with no time passing.
#[gpui::test]
fn a_short_window_gives_the_region_the_whole_window_without_waiting(cx: &mut TestAppContext) {
    let (mut helm, cx, _terminal) = open(cx, &[]);
    cx.simulate_resize(size(px(1200.), px(150.)));
    settle(cx);
    push(&mut helm, &[stop("a", "box")], &[]);
    settle(cx);
    let region = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("approval-region"))
        .expect("the region is drawn");
    assert_eq!(
        region.bounds.origin.y,
        px(0.),
        "the region took the whole window"
    );
}
